//! Variables marked `{attribute 'noinit'}` (specification 10 §4.8, specification 44 §6.7), in a module of
//! their own so that the CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

// Specification 10 §4.8 and specification 44 §6.7: a variable marked {attribute 'noinit'}, also
// spelled no_init and no-init, keeps its value at a cold and a warm restart, as CODESYS keeps it at
// a reset ("the variable iB retains its current value"), under every profile; it takes its starting
// value when the program is installed. Worked by hand, each cycle adding 1 to every variable:
// installed (iA 0, iB 7, gPlain 5, gKept 5, plain 0, kept 0), two cycles give (2, 9, 7, 7, 2, 2); a
// warm restart starts the plain ones again, so a cycle gives (1, 10, 6, 8, 1, 3); a cold restart the
// same, (1, 11, 6, 9, 1, 4); an online change carries every value and a cold restart after it keeps
// the marked ones, so a cycle gives (1, 12, 6, 10, 1, 5). Catches the mark ignored (iB 8 after the
// warm restart), its starting value not given at the install (iB 2 after two cycles), the cold
// restart initializing it, a function block's marked output or a marked global not kept, and the
// program after an online change taken for one never started (iB 8 at the end).
const NOINIT: &str = r#"
VAR_GLOBAL
    gPlain : INT := 5;
    {attribute 'no_init'}
    gKept : INT := 5;
END_VAR

FUNCTION_BLOCK FB_Count
VAR_OUTPUT
    plain : INT;
    {attribute 'noinit'}
    kept : INT;
END_VAR
plain := plain + 1;
kept := kept + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    iA : INT;
    {attribute 'no-init'}
    iB : INT := 7;
    counter : FB_Count;
    gPlainSeen : INT;
    gKeptSeen : INT;
    plainSeen : INT;
    keptSeen : INT;
END_VAR
iA := iA + 1;
iB := iB + 1;
gPlain := gPlain + 1;
gKept := gKept + 1;
counter();
gPlainSeen := gPlain;
gKeptSeen := gKept;
plainSeen := counter.plain;
keptSeen := counter.kept;
END_PROGRAM
"#;

fn noinit_values(harness: &mut TestHarness) -> [Option<Value>; 6] {
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    [
        "iA",
        "iB",
        "gPlainSeen",
        "gKeptSeen",
        "plainSeen",
        "keptSeen",
    ]
    .map(|name| harness.get_output(name))
}

#[test]
fn a_noinit_variable_keeps_its_value_at_a_cold_and_a_warm_restart() {
    let ints = |values: [i16; 6]| values.map(|value| Some(Value::Int(value)));
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(NOINIT).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        noinit_values(&mut harness);
        assert_eq!(
            noinit_values(&mut harness),
            ints([2, 9, 7, 7, 2, 2]),
            "{profile:?} installed"
        );
        harness
            .restart(RestartMode::Warm)
            .expect("the warm restart runs");
        assert_eq!(
            noinit_values(&mut harness),
            ints([1, 10, 6, 8, 1, 3]),
            "{profile:?} warm"
        );
        harness
            .restart(RestartMode::Cold)
            .expect("the cold restart runs");
        assert_eq!(
            noinit_values(&mut harness),
            ints([1, 11, 6, 9, 1, 4]),
            "{profile:?} cold"
        );
        harness
            .reload_source(NOINIT)
            .expect("the online change runs");
        harness
            .restart(RestartMode::Cold)
            .expect("the cold restart runs");
        assert_eq!(
            noinit_values(&mut harness),
            ints([1, 12, 6, 10, 1, 5]),
            "{profile:?} changed"
        );
    }
}

// Specification 10 §4.8: a restart's initialization still runs for a variable marked noinit, and
// what it writes into the variable is replaced by the value the variable held, while what it writes
// elsewhere stays. Worked by hand under the CODESYS profile: FB_init sets `counted` to 100 and adds
// 1 to `gInits`; installed, `counted` is 100 and a cycle makes it 101; a cold restart gives `gInits`
// its 0 and FB_init makes it 1, and `setup` takes back its 101, so a cycle gives counted 102 and
// gInits 1. Catches FB_init's write into the marked instance kept (101), and FB_init skipped at the
// restart (gInits 0).
const NOINIT_FB_INIT: &str = r#"
VAR_GLOBAL
    gInits : INT;
END_VAR

FUNCTION_BLOCK FB_Setup
VAR_OUTPUT
    counted : INT;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
counted := 100;
gInits := gInits + 1;
FB_init := TRUE;
END_METHOD
counted := counted + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    {attribute 'noinit'}
    setup : FB_Setup;
    countedSeen : INT;
    gInitsSeen : INT;
END_VAR
setup();
countedSeen := setup.counted;
gInitsSeen := gInits;
END_PROGRAM
"#;

#[test]
fn a_restart_runs_a_noinit_instances_fb_init_and_gives_the_instance_back() {
    let session = CompileSession::from_source(NOINIT_FB_INIT)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    let seen = |harness: &mut TestHarness| {
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
        (
            harness.get_output("countedSeen"),
            harness.get_output("gInitsSeen"),
        )
    };
    assert_eq!(
        seen(&mut harness),
        (Some(Value::Int(101)), Some(Value::Int(1)))
    );
    harness
        .restart(RestartMode::Cold)
        .expect("the cold restart runs");
    assert_eq!(
        seen(&mut harness),
        (Some(Value::Int(102)), Some(Value::Int(1)))
    );
}
