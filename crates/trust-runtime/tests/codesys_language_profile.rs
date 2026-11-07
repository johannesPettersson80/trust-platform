use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

// Specification 14: the project's `vendor_profile` selects the language rules of the runtime build
// with its conversion rules. A function block that assigns its own input in its body is refused
// under the IEC rules (IEC 61131-3 Ed.3 Figure 13 rule 2a) and built under the CODESYS profile
// (docs/IEC_DEVIATIONS.md DEV-029).
const SOURCE: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    increment : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
total := total + increment;
increment := 0;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    counter : Counter;
    first : INT;
    second : INT;
END_VAR
counter(increment := 5);
first := counter.total;
counter();
second := counter.total;
END_PROGRAM
"#;

// Worked by hand: the first call adds 5 and the body clears `increment`; the second call assigns no
// input, so the block keeps the 0 its body left and the total stays 5. Catches the CODESYS profile
// not reaching the runtime build (refused as under IEC), and a body's input assignment lost (the
// second call would add 5 again, giving 10).
#[test]
fn the_codesys_profile_builds_a_block_that_assigns_its_own_input() {
    let session =
        CompileSession::from_source(SOURCE).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    harness.cycle();
    assert_eq!(harness.get_output("first"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(5)));
}

// Catches the CODESYS rule applied to a project under the IEC rules.
#[test]
fn the_iec_rules_refuse_a_block_that_assigns_its_own_input() {
    let Err(error) = TestHarness::from_session(CompileSession::from_source(SOURCE)) else {
        panic!("the IEC rules refuse the input assignment");
    };
    let text = error.to_string();
    assert!(
        text.contains("cannot assign to input parameter 'increment'"),
        "{text}"
    );
}

/// The program of `source` built under the CODESYS profile and run for `cycles` cycles.
fn codesys_run(source: &str, cycles: usize) -> TestHarness {
    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for _ in 0..cycles {
        harness.cycle();
    }
    harness
}

// DEV-028: under the CODESYS profile another POU writes and reads a function block's `VAR` member
// through the instance. Worked by hand: Main sets `stored` to 10, the call adds 1, Main reads 11.
// Catches the write not reaching the instance (the call would read 0 and give 1) and the read
// refused by the build.
#[test]
fn a_function_block_member_is_written_and_read_from_outside() {
    let harness = codesys_run(
        r#"
FUNCTION_BLOCK Device
VAR
    stored : INT;
END_VAR
stored := stored + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    device : Device;
    seen : INT;
END_VAR
device.stored := 10;
device();
seen := device.stored;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("seen"), Some(Value::Int(11)));
}

// DEV-030: under the CODESYS profile a program is one instance named by its type. Without a
// CONFIGURATION each program runs as one instance of its name (specification 18 §2.2), and
// `Producer.counter` is that instance's variable. Worked by hand: Producer's body is empty, so
// each of Main's cycles adds 10: 10 after one cycle, 20 after two. Catches `Producer.counter`
// built as a variable of its own (Main would see its own 10 each cycle) and the access refused.
#[test]
fn a_program_variable_is_reached_by_the_program_name() {
    let harness = codesys_run(
        r#"
PROGRAM Producer
VAR
    counter : INT;
END_VAR
END_PROGRAM

PROGRAM Main
VAR
    seen : INT;
END_VAR
Producer.counter := Producer.counter + 10;
seen := Producer.counter;
END_PROGRAM
"#,
        2,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(20)));
    assert_eq!(harness.get_output("Producer.counter"), Some(Value::Int(20)));
}

// The `REF=` rebinding of specification 06 with its source through the program's name: the shape a
// configuration uses to hand each consumer the storage it binds behind its reference.
const CROSS_PROGRAM_REF_SOURCE: &str = r#"
PROGRAM Producer
VAR
    n : INT := 5;
END_VAR
END_PROGRAM

PROGRAM Main
VAR
    r : REFERENCE TO INT;
    seen : INT;
END_VAR
r REF= Producer.n;
seen := r^;
r^ := 7;
END_PROGRAM
"#;

// DEV-030 with the `REF=` extension (specification 06): the rebinding names the other program's
// variable, not a copy of it. Worked by hand: Main binds r to Producer.n (5), reads 5, and the
// write through r^ lands in Producer's own storage. Catches the source built as a copy (Producer.n
// stays 5) and the qualified source refused by the build.
#[test]
fn a_reference_rebinds_to_another_programs_variable() {
    let harness = codesys_run(CROSS_PROGRAM_REF_SOURCE, 1);
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("Producer.n"), Some(Value::Int(7)));
}

// The segmented shapes: an indexed function-block array element's member as the source, and a
// global array element's reference-typed member as the target (the event-wiring idiom). Worked by
// hand: afb[2].stClick.count = 42 and stLocal.count = 41, so Main reads 42 through rLocal and 41
// through the global holder's reference. Catches a segmented or indexed source refused, or the
// global target's rebinding stored anywhere but the global's own storage.
#[test]
fn a_reference_rebinds_through_segmented_and_indexed_paths() {
    let harness = codesys_run(
        r#"
TYPE Event : STRUCT count : INT; END_STRUCT END_TYPE
TYPE Click : STRUCT stClick : Event; END_STRUCT END_TYPE
TYPE Holder : STRUCT pEvent : REFERENCE TO Event; END_STRUCT END_TYPE

VAR_GLOBAL
    entities : ARRAY[0..1] OF Holder;
END_VAR

PROGRAM Producer
VAR
    stLocal : Event;
    afb : ARRAY[0..3] OF Click;
END_VAR
stLocal.count := 41;
afb[2].stClick.count := 42;
END_PROGRAM

PROGRAM Main
VAR
    rLocal : REFERENCE TO Event;
    seen : INT;
    viaGlobal : INT;
END_VAR
rLocal REF= Producer.afb[2].stClick;
seen := rLocal^.count;
entities[0].pEvent REF= Producer.stLocal;
viaGlobal := entities[0].pEvent^.count;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(42)));
    assert_eq!(harness.get_output("Main.viaGlobal"), Some(Value::Int(41)));
}

// DEV-030: the IEC rules have no access to a program through its name (IEC 61131-3 Ed.3 §6.6.4
// item 3), so its `REF=` source is refused too — the CODESYS twin above builds the same source.
// Catches the profile rule bypassed by the rebinding.
#[test]
fn the_iec_rules_refuse_a_reference_bound_to_another_programs_variable() {
    let Err(error) =
        TestHarness::from_session(CompileSession::from_source(CROSS_PROGRAM_REF_SOURCE))
    else {
        panic!("the IEC rules refuse `r REF= Producer.n`");
    };
    let text = error.to_string();
    assert!(text.contains("Producer"), "{text}");
}

// DEV-030: under the CODESYS profile `Doubler(x := 21)` runs Doubler's body with that input.
// Worked by hand: y = 21 * 2 = 42 whichever program the cycle runs first (Doubler's own run keeps
// the input 21 Main passed). Catches the call refused, or run without its argument (y = 0).
#[test]
fn a_program_is_called_by_its_name() {
    let harness = codesys_run(
        r#"
PROGRAM Doubler
VAR_INPUT
    x : INT;
END_VAR
VAR_OUTPUT
    y : INT;
END_VAR
y := x * 2;
END_PROGRAM

PROGRAM Main
VAR
    seen : INT;
END_VAR
Doubler(x := 21);
seen := Doubler.y;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(42)));
}

// DEV-031: under the CODESYS profile a reference in value position is the value it references.
// Worked by hand: `r.x := 7` writes p.x, read back 7; `ra[2]` is a[2] = 3; `copied := rn` reads
// n = 4; `rn := 9` writes n; `sum := rn + 1` is 9 + 1 = 10. Catches a reference copied instead of
// dereferenced, a write landing in the reference instead of the variable, and the build refusing.
#[test]
fn references_are_dereferenced_without_a_caret() {
    let harness = codesys_run(
        r#"
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT END_TYPE
TYPE Triple : ARRAY[0..2] OF INT; END_TYPE

PROGRAM Main
VAR
    p : Point;
    r : REFERENCE TO Point;
    a : Triple := [1, 2, 3];
    ra : REFERENCE TO Triple;
    n : INT := 4;
    rn : REF_TO INT;
    viaField : INT;
    viaIndex : INT;
    copied : INT;
    sum : INT;
END_VAR
r REF= p;
r.x := 7;
viaField := r.x;
ra REF= a;
viaIndex := ra[2];
rn REF= n;
copied := rn;
rn := 9;
sum := rn + 1;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("viaField"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("p.x"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("viaIndex"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("copied"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("n"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("sum"), Some(Value::Int(10)));
}

// DEV-032: under the CODESYS profile an input bound to nothing keeps its value and an output bound
// to nothing is discarded. Worked by hand: `factor` keeps its initial 3, so result = 2 * 3 = 6;
// `doubled` goes nowhere, so `d` keeps -1. Catches the empty input taken as 0 (result 0) and the
// build refusing the empty bindings.
#[test]
fn an_empty_binding_leaves_the_input_and_discards_the_output() {
    let harness = codesys_run(
        r#"
FUNCTION_BLOCK Scale
VAR_INPUT
    value : INT;
    factor : INT := 3;
END_VAR
VAR_OUTPUT
    result : INT;
    doubled : INT;
END_VAR
result := value * factor;
doubled := value * 2;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    scale : Scale;
    r : INT;
    d : INT := -1;
END_VAR
scale(value := 2, factor := , result => r, doubled => );
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("r"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("d"), Some(Value::Int(-1)));
}

// PR #140's extensions that change no IEC program build under both profiles: `REF=`, the
// `REFERENCE TO` spelling, `EnumType.Member`, an enum-typed subscript and `TYPE … EXTENDS` on a
// structure. Worked by hand: `Axis.Y` is the second value, so `table[which]` is table[1] = 20;
// `base := derived` copies the base part, a = 5. Catches an extension that only the analysis
// accepts and the build refuses or computes differently.
#[test]
fn the_extensions_build_and_run_under_both_profiles() {
    let source = r#"
TYPE Axis : (X, Y, Z); END_TYPE
TYPE Base : STRUCT a : INT; END_STRUCT END_TYPE
TYPE Derived EXTENDS Base : STRUCT b : INT; END_STRUCT END_TYPE

PROGRAM Main
VAR
    table : ARRAY[0..2] OF INT := [10, 20, 30];
    which : Axis := Axis.Y;
    picked : INT;
    derived : Derived;
    base : Base;
    gotA : INT;
    n : INT := 4;
    rn : REFERENCE TO INT;
    viaDeref : INT;
END_VAR
picked := table[which];
derived.a := 5;
derived.b := 6;
base := derived;
gotA := base.a;
rn REF= n;
viaDeref := rn^;
END_PROGRAM
"#;
    for conversions in [ConversionProfile::Iec, ConversionProfile::Codesys] {
        let session = CompileSession::from_source(source).with_conversion_profile(conversions);
        let mut harness = TestHarness::from_session(session).expect("both profiles build it");
        harness.cycle();
        assert_eq!(
            harness.get_output("picked"),
            Some(Value::Int(20)),
            "{conversions:?}"
        );
        assert_eq!(
            harness.get_output("gotA"),
            Some(Value::Int(5)),
            "{conversions:?}"
        );
        assert_eq!(
            harness.get_output("viaDeref"),
            Some(Value::Int(4)),
            "{conversions:?}"
        );
    }
}

// Specification 02 §4.1: `__ISVALIDREF(ref)` is TRUE once a reference is bound and FALSE while it is
// NULL, for a `REFERENCE TO` and a `REF_TO`, under the CODESYS and the IEC profiles. Worked by hand:
// both references start NULL (FALSE, FALSE over flags that started TRUE), are bound in the cycle
// (TRUE, TRUE), and `byRef := NULL` unbinds one again (FALSE). Catches the test inverted, NULL taken
// as bound, and the result fixed by the first binding.
const IS_VALID_REF: &str = r#"
PROGRAM Main
VAR
    value : INT;
    byName : REFERENCE TO INT;
    byRef : REF_TO INT;
    nameBefore : BOOL := TRUE;
    refBefore : BOOL := TRUE;
    nameAfter : BOOL;
    refAfter : BOOL;
    refCleared : BOOL := TRUE;
END_VAR
nameBefore := __ISVALIDREF(byName);
refBefore := __ISVALIDREF(byRef);
byName REF= value;
byRef := REF(value);
nameAfter := __ISVALIDREF(byName);
refAfter := __ISVALIDREF(byRef);
byRef := NULL;
refCleared := __ISVALIDREF(byRef);
END_PROGRAM
"#;

#[test]
fn is_valid_ref_tells_a_bound_reference_from_null() {
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(IS_VALID_REF).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        for (name, expected) in [
            ("nameBefore", false),
            ("refBefore", false),
            ("nameAfter", true),
            ("refAfter", true),
            ("refCleared", false),
        ] {
            assert_eq!(
                harness.get_output(name),
                Some(Value::Bool(expected)),
                "{profile:?}: {name}"
            );
        }
    }
}

// Specification 04: `__QUERYINTERFACE(source, target)` is `target ?= source` then `target <> NULL`,
// under the CODESYS and the IEC profiles. Worked by hand: the lamp implements IDimmer, so the first
// query is TRUE and the dimmer gives the lamp's level 3; it does not implement IFan, so the second
// query is FALSE and the fan, which named another block, is NULL afterwards. Catches the answer
// inverted, the target left unchanged on a failed query (CODESYS sets it to 0), and a query that
// never names the instance.
const QUERY_INTERFACE: &str = r#"
INTERFACE ILamp
METHOD Toggle : BOOL
END_METHOD
END_INTERFACE

INTERFACE IDimmer
METHOD Dim : INT
END_METHOD
END_INTERFACE

INTERFACE IFan
METHOD Spin : BOOL
END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Lamp IMPLEMENTS ILamp, IDimmer
VAR
    level : INT := 3;
END_VAR
METHOD PUBLIC Toggle : BOOL
Toggle := TRUE;
END_METHOD
METHOD PUBLIC Dim : INT
Dim := level;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Fan IMPLEMENTS IFan
METHOD PUBLIC Spin : BOOL
Spin := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    lamp : FB_Lamp;
    fanBlock : FB_Fan;
    lampItf : ILamp;
    dimmer : IDimmer;
    fan : IFan;
    found : BOOL;
    level : INT;
    missing : BOOL := TRUE;
    fanCleared : BOOL;
END_VAR
lampItf := lamp;
found := __QUERYINTERFACE(lampItf, dimmer);
IF found THEN
    level := dimmer.Dim();
END_IF
fan := fanBlock;
missing := __QUERYINTERFACE(lampItf, fan);
fanCleared := fan = NULL;
END_PROGRAM
"#;

#[test]
fn query_interface_answers_and_sets_the_target() {
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(QUERY_INTERFACE).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        assert_eq!(
            harness.get_output("found"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("level"),
            Some(Value::Int(3)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("missing"),
            Some(Value::Bool(false)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("fanCleared"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
    }
}
