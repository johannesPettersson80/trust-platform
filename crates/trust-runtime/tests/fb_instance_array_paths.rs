//! Calls of function block instances in arrays reached through longer paths (#163 item 1): through structure
//! members, a namespaced global list and, under the CODESYS profile, another program (IEC 61131-3 Ed.3 6.6.3.4.1
//! example e); docs/specs/02-data-types.md §3.4, docs/specs/04-pou-declarations.md Table 42,
//! docs/IEC_DEVIATIONS.md DEV-030). The plain `Sums[I](...)` shapes are in fb_instance_arrays.rs.

use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

const ACCUMULATOR: &str = r#"
FUNCTION_BLOCK Accumulator
VAR_INPUT
    Increment : DINT;
END_VAR
VAR_OUTPUT
    Total : DINT;
END_VAR
Total := Total + Increment;
END_FUNCTION_BLOCK
"#;

fn run(source: &str, cycles: usize) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile and initialize");
    for _ in 0..cycles {
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    harness
}

// The segmented paths of #163 item 1: an instance array reached through members, a namespace or another
// program, as vendor projects keep per-zone and per-device function block tables. Each call must reach its own
// element; expected values over two cycles, worked by hand.

#[test]
fn an_instance_array_inside_a_structure_array_is_called_through_its_path() {
    // Zones[I].Sums[J] adds I * 10 + J each cycle: after two cycles Zones[0].Sums[1] holds 2 * 1 = 2 and
    // Zones[1].Sums[2] holds 2 * 12 = 24. Catches a path call that reaches another element or no element.
    let source = format!(
        "{ACCUMULATOR}
TYPE Zone : STRUCT Sums : ARRAY[0..2] OF Accumulator; END_STRUCT END_TYPE
PROGRAM Main
VAR
    Zones : ARRAY[0..1] OF Zone;
    I : DINT;
    J : DINT;
    ZeroOne : DINT;
    OneTwo : DINT;
END_VAR
FOR I := 0 TO 1 DO
    FOR J := 0 TO 2 DO
        Zones[I].Sums[J](Increment := I * 10 + J);
    END_FOR;
END_FOR;
ZeroOne := Zones[0].Sums[1].Total;
OneTwo := Zones[1].Sums[2].Total;
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("ZeroOne"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("OneTwo"), Some(Value::DInt(24)));
}

#[test]
fn an_instance_array_of_a_namespaced_global_list_is_called_by_subscript() {
    // GVL.Devices[I] adds I + 1 each cycle: after two cycles element 0 holds 2 and element 3 holds 8. Catches a
    // qualified global path that calls a copy or misses the subscript.
    let source = format!(
        "{ACCUMULATOR}
NAMESPACE GVL
VAR_GLOBAL
    Devices : ARRAY[0..3] OF Accumulator;
END_VAR
END_NAMESPACE
PROGRAM Main
VAR
    I : DINT;
    First : DINT;
    Last : DINT;
END_VAR
FOR I := 0 TO 3 DO
    GVL.Devices[I](Increment := I + 1);
END_FOR;
First := GVL.Devices[0].Total;
Last := GVL.Devices[3].Total;
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("First"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("Last"), Some(Value::DInt(8)));
}

#[test]
fn a_function_block_member_of_a_structure_array_element_is_called() {
    // Cells[I].Ctrl adds Cells[I].Gain (1, 2, 3) each cycle: after two cycles Cells[1].Ctrl holds 4. Catches the
    // member call reaching the wrong element's instance or reading another element's input.
    let source = format!(
        "{ACCUMULATOR}
TYPE Cell : STRUCT Ctrl : Accumulator; Gain : DINT; END_STRUCT END_TYPE
PROGRAM Main
VAR
    Cells : ARRAY[0..2] OF Cell;
    I : DINT;
    Second : DINT;
END_VAR
Cells[0].Gain := 1;
Cells[1].Gain := 2;
Cells[2].Gain := 3;
FOR I := 0 TO 2 DO
    Cells[I].Ctrl(Increment := Cells[I].Gain);
END_FOR;
Second := Cells[1].Ctrl.Total;
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(4)));
}

#[test]
fn another_programs_instance_array_is_called_under_the_codesys_profile() {
    // DEV-030: under the CODESYS profile a program's name qualifies its members, so Main calls Producer's
    // elements. Each call adds 5: after two cycles Producer.Sums[1] holds 10. Catches the qualified path calling
    // a copy (Producer's own element staying 0) or refused by the build.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Producer
VAR
    Sums : ARRAY[0..1] OF Accumulator;
END_VAR
END_PROGRAM
PROGRAM Main
VAR
    I : DINT;
    Seen : DINT;
END_VAR
FOR I := 0 TO 1 DO
    Producer.Sums[I](Increment := 5);
END_FOR;
Seen := Producer.Sums[1].Total;
END_PROGRAM
"
    );
    let session =
        CompileSession::from_source(&source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for _ in 0..2 {
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    assert_eq!(harness.get_output("Main.Seen"), Some(Value::DInt(10)));
}

// The vendor case of #163 item 1 (mikolaj-kow, from the lighting project's mirror): the subscript is itself a
// member read, `afbDim[pE.uiDriverIdx](...)`, with an output written into an array element. A Dimmer counts its
// calls and passes its level out.
const DIMMER: &str = r#"
TYPE Entity : STRUCT uiDriverIdx : INT; END_STRUCT END_TYPE

FUNCTION_BLOCK Dimmer
VAR_INPUT
    Level : INT;
END_VAR
VAR_OUTPUT
    Out : INT;
    Calls : INT;
END_VAR
Out := Level;
Calls := Calls + 1;
END_FUNCTION_BLOCK
"#;

#[test]
fn a_member_read_subscript_selects_the_element_it_names() {
    // entities[0] names driver 2 and entities[1] driver 3; each cycle calls afbDim[2] with level 10 and
    // afbDim[3] with level 20, so after two cycles each holds 2 calls, driver 3's output is 20 and results[3]
    // took it through `Out =>`. Catches a member-read subscript evaluated as another element (or element 0).
    let source = format!(
        "{DIMMER}
PROGRAM Main
VAR
    afbDim : ARRAY[0..3] OF Dimmer;
    entities : ARRAY[0..1] OF Entity;
    results : ARRAY[0..3] OF INT;
    i : INT;
    callsAt0 : INT;
    callsAt2 : INT;
    callsAt3 : INT;
    resultAt3 : INT;
END_VAR
entities[0].uiDriverIdx := 2;
entities[1].uiDriverIdx := 3;
FOR i := 0 TO 1 DO
    afbDim[entities[i].uiDriverIdx](Level := 10 * (i + 1), Out => results[entities[i].uiDriverIdx]);
END_FOR;
callsAt0 := afbDim[0].Calls;
callsAt2 := afbDim[2].Calls;
callsAt3 := afbDim[3].Calls;
resultAt3 := results[3];
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("callsAt0"), Some(Value::Int(0)));
    assert_eq!(harness.get_output("callsAt2"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("callsAt3"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("resultAt3"), Some(Value::Int(20)));
}

#[test]
fn a_subscript_read_through_a_reference_selects_the_element_under_the_codesys_profile() {
    // The vendor spelling: pE is a REFERENCE TO the current entity, so `pE.uiDriverIdx` reads through the reference.
    // Same plan as above: after two cycles afbDim[2] and afbDim[3] hold 2 calls each and results[3] is 20. Catches
    // the reference read as the reference itself, or bound to a stale entity.
    let source = format!(
        "{DIMMER}
PROGRAM Main
VAR
    afbDim : ARRAY[0..3] OF Dimmer;
    entities : ARRAY[0..1] OF Entity;
    results : ARRAY[0..3] OF INT;
    pE : REFERENCE TO Entity;
    i : INT;
    callsAt2 : INT;
    callsAt3 : INT;
    resultAt3 : INT;
END_VAR
entities[0].uiDriverIdx := 2;
entities[1].uiDriverIdx := 3;
FOR i := 0 TO 1 DO
    pE REF= entities[i];
    afbDim[pE.uiDriverIdx](Level := 10 * (i + 1), Out => results[pE.uiDriverIdx]);
END_FOR;
callsAt2 := afbDim[2].Calls;
callsAt3 := afbDim[3].Calls;
resultAt3 := results[3];
END_PROGRAM
"
    );
    let session =
        CompileSession::from_source(&source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for _ in 0..2 {
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    assert_eq!(harness.get_output("callsAt2"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("callsAt3"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("resultAt3"), Some(Value::Int(20)));
}
