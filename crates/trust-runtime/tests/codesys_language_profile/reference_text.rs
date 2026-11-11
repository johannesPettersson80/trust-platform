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
