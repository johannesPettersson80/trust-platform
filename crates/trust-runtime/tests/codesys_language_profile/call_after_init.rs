//! Methods marked `{attribute 'call_after_init'}` (specification 10 §4.8), in a module of their own so
//! that the CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

// Specification 10 §4.8: under the CODESYS profiles a restart calls a method marked
// {attribute 'call_after_init'} right after its instance's FB_init, when the block is marked too, as
// the vendor's help says ("after the FB_Init method and after the variable value of an
// initialization expression has been valid"). Worked by hand: FB_init adds the digit 1 to `trace`
// and the marked method 2, so `trace` is 12, and the method sees `in1` as the instance's
// initializer gives it, 99; a warm restart initializes `trace` again and gives 12 again. The method
// marked in a block that is not marked never runs, and under the IEC profile nothing runs. Catches
// the method not called, called before FB_init (21), called before the initializer's value is in
// place (0), called for a block without the mark, and called under the IEC profile.
const CALL_AFTER_INIT: &str = r#"
{attribute 'call_after_init'}
FUNCTION_BLOCK FB_Marked
VAR_INPUT
    in1 : INT;
END_VAR
VAR_OUTPUT
    trace : INT;
    seenIn : INT;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + 1;
FB_init := TRUE;
END_METHOD

{attribute 'call_after_init'}
METHOD FB_AfterInit
trace := trace * 10 + 2;
seenIn := in1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Unmarked
VAR_OUTPUT
    called : BOOL;
END_VAR
{attribute 'call_after_init'}
METHOD AfterInit
called := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    marked : FB_Marked := (in1 := 99);
    unmarked : FB_Unmarked;
    traceSeen : INT;
    inSeen : INT;
    unmarkedSeen : BOOL;
END_VAR
traceSeen := marked.trace;
inSeen := marked.seenIn;
unmarkedSeen := unmarked.called;
END_PROGRAM
"#;

#[test]
fn a_call_after_init_method_runs_after_fb_init_when_its_block_is_marked() {
    for (profile, trace, seen) in [
        (ConversionProfile::Codesys, 12, 99),
        (ConversionProfile::Iec, 0, 0),
    ] {
        let session = CompileSession::from_source(CALL_AFTER_INIT).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        for restart in ["cold", "warm"] {
            if restart == "warm" {
                harness
                    .restart(RestartMode::Warm)
                    .expect("the warm restart runs");
            }
            let cycle = harness.cycle();
            assert!(
                cycle.errors.is_empty(),
                "{profile:?} {restart}: {:?}",
                cycle.errors
            );
            assert_eq!(
                harness.get_output("traceSeen"),
                Some(Value::Int(trace)),
                "{profile:?} {restart}"
            );
            assert_eq!(
                harness.get_output("inSeen"),
                Some(Value::Int(seen)),
                "{profile:?} {restart}"
            );
            assert_eq!(
                harness.get_output("unmarkedSeen"),
                Some(Value::Bool(false)),
                "{profile:?} {restart}"
            );
        }
    }
}

// Specification 10 §4.8: a method marked {attribute 'call_after_init'} takes no inputs, which the
// vendor refuses ("VAR_INPUT declarations ... lead to compile errors"), so the build refuses one.
// Catches the build calling it with made-up inputs.
#[test]
fn a_call_after_init_method_with_an_input_is_refused_at_build() {
    let source = r#"
{attribute 'call_after_init'}
FUNCTION_BLOCK FB_Bad
VAR
    x : INT;
END_VAR
{attribute 'call_after_init'}
METHOD AfterInit
VAR_INPUT
    value : INT;
END_VAR
x := value;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    bad : FB_Bad;
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
            .contains("FB_Bad.AfterInit with inputs: a method called after the initialization takes no inputs"),
        "{error}"
    );
}
