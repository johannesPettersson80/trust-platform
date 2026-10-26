use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn case_ranges_match_unsigned_and_bit_string_selectors() {
    let source = r#"
PROGRAM Main
VAR
    u : USINT := USINT#3;
    ud : UDINT := UDINT#70000;
    b : BYTE := BYTE#16#20;
    hit_u : INT;
    hit_ud : INT;
    hit_b : INT;
    hit_untyped : INT;
END_VAR
CASE u OF
    USINT#1..USINT#5: hit_u := 1;
ELSE
    hit_u := -1;
END_CASE
CASE ud OF
    UDINT#0..UDINT#65535: hit_ud := 1;
    UDINT#65536..UDINT#100000: hit_ud := 2;
END_CASE
CASE b OF
    BYTE#16#00..BYTE#16#1F: hit_b := 1;
    BYTE#16#20..BYTE#16#FF: hit_b := 2;
END_CASE
CASE u OF
    0..2: hit_untyped := 1;
    3..255: hit_untyped := 2;
END_CASE
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("hit_u"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("hit_ud"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("hit_b"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("hit_untyped"), Some(Value::Int(2)));
}

const INTEGER_AND_BIT_STRING_TYPES: [&str; 12] = [
    "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "BYTE", "WORD", "DWORD",
    "LWORD",
];

// IEC 61131-3 Ed.3 7.3.3.3.3: the selector is an expression of an elementary type and the
// labels are literals or constants of a matching type. For every integer and bit-string
// selector: untyped and typed single labels, untyped and typed ranges, and ELSE.
#[test]
fn case_labels_match_every_integer_and_bit_string_selector_type() {
    let mut declarations = String::new();
    let mut statements = String::new();
    for ty in INTEGER_AND_BIT_STRING_TYPES {
        let v = format!("v_{ty}");
        declarations.push_str(&format!(
            "    {v} : {ty} := {ty}#7;\n    single_{ty} : INT;\n    typed_{ty} : INT;\n    \
             range_{ty} : INT;\n    typed_range_{ty} : INT;\n    other_{ty} : INT;\n"
        ));
        statements.push_str(&format!(
            "CASE {v} OF 6: single_{ty} := 1; 7: single_{ty} := 2; ELSE single_{ty} := -1; END_CASE\n\
             CASE {v} OF {ty}#6: typed_{ty} := 1; {ty}#7: typed_{ty} := 2; ELSE typed_{ty} := -1; END_CASE\n\
             CASE {v} OF 0..6: range_{ty} := 1; 7..9: range_{ty} := 2; ELSE range_{ty} := -1; END_CASE\n\
             CASE {v} OF {ty}#0..{ty}#6: typed_range_{ty} := 1; {ty}#7..{ty}#9: typed_range_{ty} := 2; \
             ELSE typed_range_{ty} := -1; END_CASE\n\
             CASE {v} OF 0..6, 8: other_{ty} := 1; ELSE other_{ty} := -1; END_CASE\n"
        ));
    }
    let source = format!("PROGRAM Main\nVAR\n{declarations}END_VAR\n{statements}END_PROGRAM\n");

    let mut harness = TestHarness::from_source(&source).unwrap();
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    for ty in INTEGER_AND_BIT_STRING_TYPES {
        for (prefix, expected) in [
            ("single", 2),
            ("typed", 2),
            ("range", 2),
            ("typed_range", 2),
            ("other", -1),
        ] {
            let name = format!("{prefix}_{ty}");
            assert_eq!(
                harness.get_output(&name),
                Some(Value::Int(expected)),
                "{name}"
            );
        }
    }
}

#[test]
fn case_labels_match_enumeration_and_character_string_selectors() {
    // A string label is compared with its full text: a label longer than the selector's
    // STRING[3] matches no value of it, and in particular not the one it starts with.
    let source = r#"
TYPE Mode : (Idle, Run, Fault); END_TYPE

PROGRAM Main
VAR
    m : Mode := Mode#Fault;
    short : STRING[3] := 'ABC';
    long : STRING[10] := 'ABCD';
    wide : WSTRING[3] := "ABC";
    letter : CHAR := CHAR#'B';
    by_mode : INT;
    by_short : INT;
    by_long : INT;
    by_wide : INT;
    by_letter : INT;
END_VAR
CASE m OF
    Idle, Run: by_mode := 1;
    Fault: by_mode := 2;
ELSE
    by_mode := -1;
END_CASE
CASE short OF
    'ABCD': by_short := 1;
    'ABC': by_short := 2;
ELSE
    by_short := -1;
END_CASE
CASE long OF
    'ABC': by_long := 1;
    'ABCD': by_long := 2;
ELSE
    by_long := -1;
END_CASE
CASE wide OF
    "ABCD": by_wide := 1;
    "ABC": by_wide := 2;
ELSE
    by_wide := -1;
END_CASE
CASE letter OF
    CHAR#'A': by_letter := 1;
    CHAR#'B': by_letter := 2;
ELSE
    by_letter := -1;
END_CASE
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    for name in ["by_mode", "by_short", "by_long", "by_wide", "by_letter"] {
        assert_eq!(harness.get_output(name), Some(Value::Int(2)), "{name}");
    }
}
