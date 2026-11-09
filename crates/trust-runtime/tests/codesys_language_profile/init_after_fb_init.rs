//! An instance's declared values given after its FB_init, under the CODESYS profiles (specification 10 §4.8), in a
//! module of their own so that the CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

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
