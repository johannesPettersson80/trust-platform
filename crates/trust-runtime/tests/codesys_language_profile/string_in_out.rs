//! A string `VAR_IN_OUT` of a function or a method takes a string of its family of any capacity
//! (docs/IEC_DEVIATIONS.md DEV-035), seen through the capacity it declares (specification 44 §7.3,
//! "A string `VAR_IN_OUT` of a function or a method takes a variable of its family …").

use super::*;

/// `Fill` reads its in-out's length and writes eight characters into it, from a variable (a literal
/// longer than the in-out is refused at compile time, E304); `Main` gives it a string of a larger
/// and of a smaller capacity than the five the in-out declares.
const ANY_CAPACITY: &str = r#"
FUNCTION Fill : INT
VAR_IN_OUT
    s : STRING(5);
END_VAR
VAR
    eight : STRING(10) := 'abcdefgh';
END_VAR
Fill := LEN(s);
s := eight;
END_FUNCTION

PROGRAM Main
VAR
    long : STRING(20) := '0123456789';
    short : STRING(3) := 'xy';
    readLong : INT;
    readShort : INT;
END_VAR
readLong := Fill(long);
readShort := Fill(short);
END_PROGRAM
"#;

// Worked by hand from the specification: through a `STRING(5)` in-out the 10 characters of `long`
// read as 5, and 'abcdefgh' is written as its first 5, 'abcde'; `short` holds 3, so its 2
// characters read as 2 and the write keeps 3, 'abc'. Catches the build that refused the binding
// ("a variable of another type", the report on #419), a write past a caller shorter than the
// in-out, and a copy that loses the callee's write.
#[test]
fn a_string_in_out_takes_a_string_of_any_capacity_through_the_capacity_it_declares() {
    let harness = codesys_run(ANY_CAPACITY, 1);
    assert_eq!(harness.get_output("readLong"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("readShort"), Some(Value::Int(2)));
    assert_eq!(
        harness.get_output("long"),
        Some(Value::String("abcde".into()))
    );
    assert_eq!(
        harness.get_output("short"),
        Some(Value::String("abc".into()))
    );
}

/// The vendor idiom of DEV-035: a `STRING(1)` in-out whose real capacity a further argument gives,
/// here of a global `STRING(500)`; the function reads only its size argument.
const VENDOR_BUFFER: &str = r#"
VAR_GLOBAL
    sTemplate : STRING(500) := 'template';
END_VAR

FUNCTION StrAppendSize : UDINT
VAR_IN_OUT
    sDest : STRING(1);
END_VAR
VAR_INPUT
    udiSize : UDINT;
    sAppend : STRING;
END_VAR
StrAppendSize := udiSize;
END_FUNCTION

PROGRAM Main
VAR
    size : UDINT;
    after : STRING(500);
END_VAR
size := StrAppendSize(sTemplate, SIZEOF(sTemplate), 'x');
after := sTemplate;
END_PROGRAM
"#;

// Worked by hand: `SIZEOF` of a `STRING(500)` is 500 (specification 29: `SIZEOF(STRING[n])` is
// `n`), and the function does not write its in-out, so the template stays 'template'. Catches the build that refused the call (the WAGO `StrAppend` site on
// #419) and an in-out passed as a copy of the declared capacity, which would cut the caller's
// string to its first character on the way back.
#[test]
fn the_vendor_buffer_idiom_builds_and_leaves_the_callers_string_alone() {
    let harness = codesys_run(VENDOR_BUFFER, 1);
    assert_eq!(harness.get_output("size"), Some(Value::UDInt(500)));
    assert_eq!(
        harness.get_output("after"),
        Some(Value::String("template".into()))
    );
}

/// `Outer` gives its in-out on to `Inner`'s, then writes the caller's string through its address.
const GIVEN_ON: &str = r#"
FUNCTION Inner : INT
VAR_IN_OUT
    s : STRING(5);
END_VAR
s := 'inner';
Inner := LEN(s);
END_FUNCTION

FUNCTION Outer : INT
VAR_IN_OUT
    s : STRING(5);
END_VAR
VAR
    p : POINTER TO STRING(5);
END_VAR
Outer := Inner(s);
p := ADR(s);
p^ := 'ptr';
END_FUNCTION

PROGRAM Main
VAR
    text : STRING(20) := 'start';
    shortText : STRING(3) := 'ab';
    n : INT;
END_VAR
n := Outer(text);
END_PROGRAM
"#;

// Worked by hand: `Inner` writes 'inner' into `text` through both in-outs and reads its 5
// characters, then `Outer` takes the address of its in-out, `text` as a `STRING(5)`, which holds
// at least 5, and writes 'ptr' through it. Catches an in-out given on as a copy, and an address of
// another variable than the caller's.
#[test]
fn a_string_in_out_given_on_and_addressed_names_the_callers_string() {
    let harness = codesys_run(GIVEN_ON, 1);
    assert_eq!(harness.get_output("n"), Some(Value::Int(5)));
    assert_eq!(
        harness.get_output("text"),
        Some(Value::String("ptr".into()))
    );
}

// Worked by hand: `shortText` holds 3 characters, fewer than the `STRING(5)` that `Outer`'s address
// names, so taking the address faults (`TypeMismatch`) and `p^ := 'ptr'` never runs; `Inner` has
// written 'inn' by then, its 'inner' cut to the 3 characters `shortText` holds. Catches an address
// whose `STRING(5)` reaches past the caller's variable into the next one.
#[test]
fn an_address_of_a_string_in_out_whose_caller_is_shorter_faults() {
    let source = GIVEN_ON.replace("n := Outer(text);", "n := Outer(shortText);");
    let session =
        CompileSession::from_source(&source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert!(
        format!("{:?}", cycle.errors[0]).contains("TypeMismatch"),
        "{:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("shortText"),
        Some(Value::String("inn".into()))
    );
}

// Specification 44 §7.3: a function block keeps a reference to a variable of its in-out's declared
// type, so a string of another capacity is not compiled for it yet (NX-09), with its place. Catches
// a block's in-out bound to a string it would read past.
#[test]
fn a_function_blocks_string_in_out_of_another_capacity_is_refused_at_build() {
    let source = r#"
FUNCTION_BLOCK Keeper
VAR_IN_OUT
    s : STRING(5);
END_VAR
s := 'kept';
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    keeper : Keeper;
    text : STRING(20);
END_VAR
keeper(s := text);
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
            .contains("a string of another capacity where a reference to a variable of the declared capacity is wanted"),
        "{error}"
    );
}
