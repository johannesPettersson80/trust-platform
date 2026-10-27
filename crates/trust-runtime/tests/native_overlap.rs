//! `STRUCT OVERLAP` with members that are structures or arrays (IEC 61131-3 §6.4.4.7.1 and its
//! example `Com_data`; specification 02 §3.6; specification 44 §6.2): each lies in the backing as
//! the bytes of its own layout, a write through one member is read through every member that
//! shares its bits, and a whole member is copied into and out of the backing member by member.
//! Every expected value is worked out by hand; the 10 ms task first runs at 10 ms (specification 10
//! §4.3); values are in the target's byte order, little-endian on every truST target.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// IEC §6.4.4.7.1's example types, as the standard writes them.
const COM_TYPES: &str = r#"
TYPE
Com1_data : STRUCT
    head AT %B0 : INT;
    length AT %B2 : USINT := 26;
    flag1 AT %X3.0 : BOOL;
    end AT %B25 : BYTE;
END_STRUCT;
Com2_data : STRUCT OVERLAP
    head AT %B0 : INT;
    length AT %B2 : USINT;
    flag2 AT %X3.3 : BOOL;
    data1 AT %B5 : BYTE;
    data2 AT %B5 : REAL;
    end AT %B19 : BYTE;
END_STRUCT;
Com_data : STRUCT OVERLAP
    C1 AT %B0 : Com1_data;
    C2 AT %B0 : Com2_data;
END_STRUCT;
END_TYPE
"#;

/// The program of `body`, its task and its configuration, compiled.
fn compiled(types: &str, body: &str) -> TestHarness {
    let source = format!(
        "{types}
{body}
CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"
    );
    TestHarness::from_source(&source).unwrap_or_else(|error| panic!("must compile: {error}"))
}

/// One activation of the 10 ms task, and the errors of its cycle.
fn activate(harness: &mut TestHarness) -> Vec<String> {
    harness.advance_time(Duration::from_millis(10));
    harness
        .cycle()
        .errors
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// IEC's `Com_data`: `C1` and `C2` lie over the same bytes. `C2.length := 9` is read as
/// `C1.length` (byte 2): 9; `C1.flag1` (bit 3.0) set leaves `C2.flag2` (bit 3.3) FALSE; `C1.end`
/// lies at byte 25, past `C2`. `Copy1 := D.C1` copies the member out: length 9, flag1 TRUE, end
/// 16#3C = 60; `D.C1 := Fresh` copies a `Com1_data` at its initial values in, so `C2.length` reads
/// its 26. Catches a structure member refused (as it was), members laid out by their own offsets
/// instead of their relative addresses (`end` at byte 4), a copy that moves the bytes of one
/// layout into the other, and `C1` and `C2` kept apart.
#[test]
fn iec_com_data_members_share_their_bytes_and_copy_member_by_member() {
    let mut harness = compiled(
        COM_TYPES,
        r#"
PROGRAM Main
VAR
    D : Com_data;
    Copy1 : Com1_data;
    Fresh : Com1_data;
    Len : USINT;
    Flag2 : BOOL;
    Back : USINT;
END_VAR
D.C2.length := USINT#9;
Len := D.C1.length;
D.C1.flag1 := TRUE;
Flag2 := D.C2.flag2;
D.C1.end := BYTE#16#3C;
Copy1 := D.C1;
D.C1 := Fresh;
Back := D.C2.length;
END_PROGRAM
"#,
    );
    let errors = activate(&mut harness);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(harness.get_output("Len"), Some(Value::USInt(9)));
    assert_eq!(harness.get_output("Flag2"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("Copy1.length"), Some(Value::USInt(9)));
    assert_eq!(harness.get_output("Copy1.flag1"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("Copy1.end"), Some(Value::Byte(0x3C)));
    assert_eq!(harness.get_output("Back"), Some(Value::USInt(26)));
}

/// An array over an `LREAL`, the bytes of a value: `1.0` is 16#3FF0000000000000, so byte 7 is
/// 16#3F = 63 and byte 6 16#F0 = 240, read with a variable and a constant index. The next scan's
/// index 8 lies outside `ARRAY[0..7]` and faults with the index and the bounds. Catches an array
/// member refused, elements at the wrong stride or byte order, and an index outside the array
/// read past it instead of faulting.
#[test]
fn an_array_over_a_value_reads_its_bytes_and_checks_its_index() {
    let mut harness = compiled(
        r#"
TYPE
Bytes8 : STRUCT OVERLAP
    raw AT %B0 : ARRAY[0..7] OF BYTE;
    value AT %B0 : LREAL;
END_STRUCT;
END_TYPE
"#,
        r#"
PROGRAM Main
VAR
    V : Bytes8;
    I : INT := 7;
    Top : BYTE;
    Low : BYTE;
END_VAR
V.value := LREAL#1.0;
Top := V.raw[I];
Low := V.raw[6];
I := I + 1;
END_PROGRAM
"#,
    );
    let errors = activate(&mut harness);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(harness.get_output("Top"), Some(Value::Byte(0x3F)));
    assert_eq!(harness.get_output("Low"), Some(Value::Byte(0xF0)));
    let errors = activate(&mut harness);
    assert!(
        errors
            .iter()
            .any(|error| error.to_ascii_lowercase().contains("index")),
        "index 8 must fault: {errors:?}"
    );
}

/// A string is no member of an overlap: "Overlapping of strings is beyond the scope of this
/// standard" (IEC §6.4.4.7.1). The build refuses it and says why. Catches a string laid over other
/// members without a word.
#[test]
fn a_string_member_of_an_overlap_is_refused_with_the_reason() {
    let source = r#"
TYPE
Texts : STRUCT OVERLAP
    text AT %B0 : STRING[4];
    code AT %B0 : DINT;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR
    T : Texts;
END_VAR
T.code := 1;
END_PROGRAM
CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let refusal = match TestHarness::from_source(source) {
        Ok(_) => panic!("the build must refuse a string in an overlap"),
        Err(error) => error.to_string(),
    };
    assert!(
        refusal.contains("the member text of STRUCT OVERLAP Texts, a string"),
        "{refusal}"
    );
}
