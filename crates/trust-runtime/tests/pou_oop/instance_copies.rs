//! A value that holds a function block instance, an instance, or a structure or an array that holds
//! one, is copied whole by an assignment and by an output bound with `=>`: every variable of the
//! source, its internal variables and the hidden state of the standard blocks it holds included,
//! goes into the target, which stays an instance of its own (specification 44 §6.2,
//! `docs/IEC_DECISIONS.md` "Copy of a function block instance"; contributor issue #451).

use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

/// `Counter` keeps its count in an internal variable; `Pair` holds a counter and a rising-edge
/// detector; `Source` gives a counter as its output.
const COPIES: &str = r#"
FUNCTION_BLOCK Counter
VAR
    count : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
count := count + 1;
total := count;
END_FUNCTION_BLOCK

TYPE Pair :
STRUCT
    counter : Counter;
    edge : R_TRIG;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Source
VAR_OUTPUT
    made : Counter;
END_VAR
made();
made();
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    a : Counter;
    b : Counter;
    p1 : Pair;
    p2 : Pair;
    row1 : ARRAY[0..1] OF Counter;
    row2 : ARRAY[0..1] OF Counter;
    source : Source;
    taken : Counter;
    bAfterCopy : INT;
    aAfterCopy : INT;
    bAgain : INT;
    edgeOfCopy : BOOL := TRUE;
    pairCount : INT;
    rowCount : INT;
    takenCount : INT;
END_VAR
a();
a();
a();
b := a;
b();
bAfterCopy := b.total;
a();
aAfterCopy := a.total;
b();
bAgain := b.total;

p1.counter();
p1.edge(CLK := TRUE);
p2 := p1;
p2.edge(CLK := TRUE);
edgeOfCopy := p2.edge.Q;
p2.counter();
pairCount := p2.counter.total;

row1[1]();
row1[1]();
row2 := row1;
row2[1]();
rowCount := row2[1].total;

source(made => taken);
taken();
takenCount := taken.total;
END_PROGRAM
"#;

// Worked by hand: `a` counts to 3, `b := a` takes its count, so `b` counts on to 4; `a`, called
// again, counts to 4 on its own, and `b` to 5. `p1`'s edge saw CLK rise, so its copy's edge, called
// with CLK still TRUE, sees no new rise: Q is FALSE; the pair's counter counts on from 1 to 2. The
// row's second counter counts on from 2 to 3. `Source` counts its output to 2, which `=> taken`
// copies, so `taken` counts on to 3. Catches the build that refused each copy (the report on #451:
// "an output that holds a function block or class instance"); a copy that misses internal variables
// (`b` would count from 0 to 1, `taken` to 1); one that misses a standard block's hidden state (the
// edge would see a rise, Q TRUE); and a copy that shares the source (`aAfterCopy` would be 5).
#[test]
fn an_instance_is_copied_whole_by_an_assignment_and_by_an_output() {
    let mut harness = TestHarness::from_source(COPIES).expect("the IEC rules build it");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("bAfterCopy", 4i16);
    harness.assert_eq("aAfterCopy", 4i16);
    harness.assert_eq("bAgain", 5i16);
    harness.assert_eq("edgeOfCopy", false);
    harness.assert_eq("pairCount", 2i16);
    harness.assert_eq("rowCount", 3i16);
    harness.assert_eq("takenCount", 3i16);
}

/// The report's shape: WAGO's status block, a function block whose methods read the error it holds,
/// given by a device block as its output `oStatus` and bound with `=>` to a variable of its type.
const REPORTED: &str = r#"
FUNCTION_BLOCK FbResult
VAR
    id : UDINT;
END_VAR
METHOD PUBLIC SetError
VAR_INPUT
    newId : UDINT;
END_VAR
id := newId;
END_METHOD
METHOD PUBLIC IsError : BOOL
IsError := id <> 0;
END_METHOD
METHOD PUBLIC GetID : UDINT
GetID := id;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FbDevice
VAR_INPUT
    fail : BOOL;
END_VAR
VAR_OUTPUT
    oStatus : FbResult;
END_VAR
IF fail THEN
    oStatus.SetError(newId := 42);
END_IF
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fbDevice : FbDevice;
    fbResult : FbResult;
    isError : BOOL;
    id : UDINT;
    idAfterSourceChanged : UDINT;
END_VAR
fbDevice(fail := TRUE, oStatus => fbResult);
isError := fbResult.IsError();
id := fbResult.GetID();
fbDevice.oStatus.SetError(newId := 7);
idAfterSourceChanged := fbResult.GetID();
END_PROGRAM
"#;

// Worked by hand: the device sets error 42 in its output, which the binding copies to `fbResult`:
// it is an error with ID 42. Setting error 7 in the device's output afterwards leaves the copy at
// 42. Catches the build that refused the binding (the report on #451) and a binding that makes
// `fbResult` name the device's output instead of copying it (the ID would read 7).
#[test]
fn the_reported_status_output_is_copied_to_its_variable_under_codesys() {
    let session =
        CompileSession::from_source(REPORTED).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("isError", true);
    harness.assert_eq("id", Value::UDInt(42));
    harness.assert_eq("idAfterSourceChanged", Value::UDInt(42));
}
