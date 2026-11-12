//! LIMIT is MIN(MAX(IN, MN), MX) (spec 07 section 6.1, IEC 61131-3 Table 32), and the selection
//! functions take texts, which compare as two texts compare (spec 07 sections 6 and 7; Table 32 types
//! MAX, MIN and LIMIT as ANY_ELEMENTARY, SEL and MUX as ANY).

use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn limit_with_inverted_bounds_returns_the_upper_bound() {
    let lib = StandardLibrary::new();

    // MN := 5, IN := 3, MX := 0: the raise to MN is capped by MX.
    assert_eq!(
        lib.call("LIMIT", &[Value::Int(5), Value::Int(3), Value::Int(0)]),
        Ok(Value::Int(0))
    );
    // MN := 5, IN := 9, MX := 0: every input resolves to MX.
    assert_eq!(
        lib.call("LIMIT", &[Value::Int(5), Value::Int(9), Value::Int(0)]),
        Ok(Value::Int(0))
    );
    assert_eq!(
        lib.call(
            "LIMIT",
            &[Value::LReal(2.0), Value::LReal(1.0), Value::LReal(1.5)]
        ),
        Ok(Value::LReal(1.5))
    );
}

#[test]
fn limit_with_ordered_bounds_clamps_both_sides() {
    let lib = StandardLibrary::new();
    let limit = |mn: i16, input: i16, mx: i16| {
        lib.call(
            "LIMIT",
            &[Value::Int(mn), Value::Int(input), Value::Int(mx)],
        )
    };

    assert_eq!(limit(0, 5, 3), Ok(Value::Int(3)));
    assert_eq!(limit(0, -5, 3), Ok(Value::Int(0)));
    assert_eq!(limit(0, 2, 3), Ok(Value::Int(2)));
    assert_eq!(limit(0, 3, 3), Ok(Value::Int(3)));
    assert_eq!(limit(3, 3, 3), Ok(Value::Int(3)));
}

/// Each selection function of texts, of `STRING`s of other capacities and literals, and of `WSTRING`s.
const TEXT_SELECTIONS: &str = r#"
PROGRAM Main
VAR
    a : STRING[8] := 'abc';
    b : STRING[16] := 'abd';
    w1 : WSTRING[8] := "ab";
    w2 : WSTRING[8] := "b";
    biggest : STRING[16];
    smallest : STRING[16];
    limited : STRING[16];
    raised : STRING[16];
    capped : STRING[16];
    chosenTrue : STRING[16];
    chosenFalse : STRING[16];
    muxed : STRING[16];
    wideMax : WSTRING[8];
    longerWins : BOOL;
END_VAR
biggest := MAX(a, b, 'ab');
smallest := MIN(a, b, 'ab');
limited := LIMIT('a', 'b', 'c');
raised := LIMIT('b', 'a', 'c');
capped := LIMIT('a', 'z', 'c');
chosenTrue := SEL(TRUE, a, b);
chosenFalse := SEL(FALSE, a, b);
muxed := MUX(1, a, b, 'zz');
wideMax := MAX(w1, w2);
longerWins := LEN(MAX(a, 'abcd')) = 4;
END_PROGRAM
"#;

// Worked by hand, texts comparing character by character and a prefix before the longer text:
// 'ab' < 'abc' < 'abd', so MAX is 'abd' and MIN 'ab'; LIMIT('a', 'b', 'c') is 'b', 'a' is raised to
// MN 'b', and 'z' is capped at MX 'c'; SEL takes IN1, 'abd', when TRUE and IN0, 'abc', when FALSE;
// MUX(1, ...) takes IN1, 'abd'; of the WSTRINGs "ab" and "b", "b" is the greater; MAX('abc',
// 'abcd') is 'abcd', of length 4. Catches the build that refused each ("a comparison of values that are
// not scalars"), a MAX or MIN that kept its first input, a LIMIT that capped before it raised, and a SEL
// or MUX that took the other input.
#[test]
fn the_selection_functions_choose_among_texts() {
    let mut harness = TestHarness::from_source(TEXT_SELECTIONS).expect("the program builds");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    for (name, expected) in [
        ("biggest", "abd"),
        ("smallest", "ab"),
        ("limited", "b"),
        ("raised", "b"),
        ("capped", "c"),
        ("chosenTrue", "abd"),
        ("chosenFalse", "abc"),
        ("muxed", "abd"),
    ] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::String(expected.into())),
            "{name}"
        );
    }
    assert_eq!(
        harness.get_output("wideMax"),
        Some(Value::WString("b".into()))
    );
    assert_eq!(harness.get_output("longerWins"), Some(Value::Bool(true)));
}

/// Under the CODESYS profile a `REFERENCE TO STRING` given to a selection function is the text it
/// names (docs/IEC_DEVIATIONS.md DEV-031).
const REFERENCE_SELECTIONS: &str = r#"
PROGRAM Main
VAR
    a : STRING[8] := 'abc';
    b : STRING[16] := 'abd';
    r : REFERENCE TO STRING[16];
    biggest : STRING[16];
    smallest : STRING[16];
END_VAR
r REF= b;
biggest := MAX(a, r);
smallest := MIN(r, a);
END_PROGRAM
"#;

// Worked by hand: `r` names 'abd', so MAX of 'abc' and it is 'abd' and MIN is 'abc'. Catches a
// reference compared as its address, or refused as no text.
#[test]
fn a_codesys_reference_to_a_string_is_selected_as_its_text() {
    let session = CompileSession::from_source(REFERENCE_SELECTIONS)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("biggest"),
        Some(Value::String("abd".into()))
    );
    assert_eq!(
        harness.get_output("smallest"),
        Some(Value::String("abc".into()))
    );
}
