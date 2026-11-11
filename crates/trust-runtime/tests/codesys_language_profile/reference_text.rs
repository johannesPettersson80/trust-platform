//! Under the CODESYS profile a reference to a `STRING` or a `WSTRING` where a text is wanted is the
//! text of the string it names (docs/IEC_DEVIATIONS.md DEV-031): as a function block's input, the
//! source of an assignment and an operand of `=` and `<>` (contributor issue #454).

use super::*;

/// The report's shape: a `REFERENCE TO STRING(uiCap)`, whose capacity a global constant gives, bound
/// to a global template, then read where a `STRING(2048)` is wanted.
const REFERENCE_TEXT: &str = r#"
VAR_GLOBAL CONSTANT
    uiCap : UINT := 2048;
END_VAR
VAR_GLOBAL
    sTemplate : STRING(uiCap) := '{"id":1}';
    wTemplate : WSTRING(20) := "wide";
END_VAR

FUNCTION_BLOCK FbWriter
VAR_INPUT
    sJson_BaseFrame : STRING(2048);
END_VAR
VAR_OUTPUT
    received : STRING(2048);
END_VAR
received := sJson_BaseFrame;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    writer : FbWriter;
    p : REFERENCE TO STRING(uiCap);
    pw : REFERENCE TO WSTRING(20);
    passed : STRING(2048) := 'old';
    sWork : STRING(2048) := 'old';
    sameAsLiteral : BOOL;
    differs : BOOL := TRUE;
    wWork : WSTRING(20);
    wSame : BOOL;
END_VAR
p REF= sTemplate;
pw REF= wTemplate;
writer(sJson_BaseFrame := p);
passed := writer.received;
sWork := p;
sameAsLiteral := p = '{"id":1}';
differs := p <> sTemplate;
wWork := pw;
wSame := pw = "wide";
END_PROGRAM
"#;

// Worked by hand: the template's text is the 8 characters `{"id":1}`. The input passes it to the
// block, which gives it back; the assignment copies it; it equals the literal and does not differ
// from the template it names. The WSTRING reference gives "wide". `passed` and `sWork` start as
// 'old', and `differs` TRUE, so a read that wrote nothing would leave them. Catches the build that
// refused each read ("a value that is no text where a text is needed", the report on #454), and a
// read of the reference's own bytes instead of the text.
#[test]
fn a_reference_to_a_string_where_a_text_is_wanted_is_the_text_it_names() {
    let harness = codesys_run(REFERENCE_TEXT, 1);
    let text = |value: &str| Some(Value::String(value.into()));
    assert_eq!(harness.get_output("passed"), text(r#"{"id":1}"#));
    assert_eq!(harness.get_output("sWork"), text(r#"{"id":1}"#));
    assert_eq!(harness.get_output("sameAsLiteral"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("differs"), Some(Value::Bool(false)));
    assert_eq!(
        harness.get_output("wWork"),
        Some(Value::WString("wide".into()))
    );
    assert_eq!(harness.get_output("wSame"), Some(Value::Bool(true)));
}

/// The string functions of specification 07 §8 given a reference to a `STRING` or a `WSTRING` as
/// their text (DEV-031: arguments use the referenced value).
const STRING_FUNCTIONS: &str = r#"
VAR_GLOBAL
    sTemplate : STRING(32) := 'id:42';
    wTemplate : WSTRING(20) := "wide";
END_VAR

PROGRAM Main
VAR
    p : REFERENCE TO STRING(32);
    pw : REFERENCE TO WSTRING(20);
    length : INT;
    wideLength : INT;
    head : STRING(32) := 'old';
    joined : STRING(40) := 'old';
    position : INT;
    middle : STRING(32) := 'old';
END_VAR
p REF= sTemplate;
pw REF= wTemplate;
length := LEN(p);
wideLength := LEN(pw);
head := LEFT(p, 2);
joined := CONCAT(p, '!');
position := FIND(p, ':');
middle := MID(p, 2, 4);
END_PROGRAM
"#;

// Worked by hand from 'id:42': it has 5 characters and "wide" 4; its first 2 are 'id'; with '!'
// appended it is 'id:42!'; ':' is at position 3; the 2 characters from position 4 are '42'. The text
// results start as 'old', so a call that wrote nothing would leave them. Catches the type checker
// refusing the reference (E205, contributor issue #454's follow-up) and a function that reads the
// reference's own bytes or the wrong family.
#[test]
fn the_string_functions_take_a_reference_to_a_string_as_its_text() {
    let harness = codesys_run(STRING_FUNCTIONS, 1);
    let text = |value: &str| Some(Value::String(value.into()));
    assert_eq!(harness.get_output("length"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("wideLength"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("head"), text("id"));
    assert_eq!(harness.get_output("joined"), text("id:42!"));
    assert_eq!(harness.get_output("position"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("middle"), text("42"));
}
