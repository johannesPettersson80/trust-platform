//! Under the CODESYS profile two integers of different families, signed, unsigned or bit string,
//! compare by their mathematical values (specification 05, "CODESYS-parity exceptions";
//! docs/IEC_DEVIATIONS.md DEV-038): `LEN(s) > uiConst`, an `INT` with a `UINT`, compiles
//! (contributor issue #452). The IEC rules refuse such a comparison.

use super::*;

/// Each pair holds values that a comparison in one of the two types would get wrong: -1 and 65535
/// share their bits in 16 bits, as 200 and -56 do in 8, and 18446744073709551615 fits no `LINT`.
const MIXED: &str = r#"
PROGRAM Main
VAR
    i : INT := -1;
    u : UINT := 65535;
    w : WORD := 16#FFFF;
    b : BYTE := 200;
    si : SINT := -56;
    di : DINT := -1;
    ud : UDINT := 4294967295;
    li : LINT := -1;
    ul : ULINT := 16#FFFFFFFFFFFFFFFF;
    u100 : UINT := 100;
    w100 : WORD := 100;
    iLessU : BOOL;
    iEqU : BOOL;
    wGreaterI : BOOL;
    bNeqSi : BOOL;
    diLessUd : BOOL;
    liLessUl : BOOL;
    uEqW : BOOL;
    uGeW : BOOL;
END_VAR
iLessU := i < u;
iEqU := i = u;
wGreaterI := w > i;
bNeqSi := b <> si;
diLessUd := di < ud;
liLessUl := li < ul;
uEqW := u100 = w100;
uGeW := u100 >= w100;
END_PROGRAM
"#;

/// Worked by hand from the values: -1 < 65535, -1 <> 65535, 65535 > -1, 200 <> -56,
/// -1 < 4294967295, -1 < 18446744073709551615, 100 = 100 and 100 >= 100.
const MIXED_EXPECTED: [(&str, bool); 8] = [
    ("iLessU", true),
    ("iEqU", false),
    ("wGreaterI", true),
    ("bNeqSi", true),
    ("diLessUd", true),
    ("liLessUl", true),
    ("uEqW", true),
    ("uGeW", true),
];

// Catches the build that refused the comparison ("a comparison of INT with UINT is not compiled
// yet", the report on #452); a comparison of the bits (-1 as 65535 makes `iEqU` TRUE and `iLessU`
// FALSE; 200 and -56 are both 16#C8, `bNeqSi` FALSE); and one in the type of either operand, which
// wraps 65535 to -1 or faults on it, and wraps 18446744073709551615 to -1 (`liLessUl` FALSE).
#[test]
fn integers_of_different_families_compare_by_value_under_codesys() {
    let harness = codesys_run(MIXED, 1);
    for (name, expected) in MIXED_EXPECTED {
        assert_eq!(
            harness.get_output(name),
            Some(Value::Bool(expected)),
            "{name}"
        );
    }
}

// IEC 61131-3 Ed.3 §6.6.1.6 converts implicitly only where value and accuracy are kept: no type takes
// both an `INT` and a `UINT`, or a `LINT` and a `ULINT`, and a bit string is no number, so the IEC
// rules refuse these comparisons (E201). Catches the CODESYS rule leaking into a project under the
// IEC rules.
#[test]
fn the_iec_rules_refuse_integers_of_different_families() {
    let Err(error) = TestHarness::from_source(MIXED) else {
        panic!("the IEC rules refuse the comparisons");
    };
    let text = error.to_string();
    assert!(
        text.contains("numeric operands have no accuracy-preserving common type"),
        "{text}"
    );
    assert!(text.contains("types are not comparable"), "{text}");
}

/// The report's own shape: `LEN` gives an `INT`, the global constants are `UINT`s. Under the
/// CODESYS profile a reference compared for equality takes part as the value it names (DEV-031),
/// so `rI`, naming the `INT` -1, compares with the `UINT` 65535 by value too.
const REPORTED: &str = r#"
VAR_GLOBAL CONSTANT
    uiLen : UINT := 150;
    uiShort : UINT := 2;
END_VAR

FUNCTION FCheck : BOOL
VAR_INPUT
    sTopic : STRING(63);
END_VAR
    IF LEN(sTopic) > uiLen THEN
        FCheck := TRUE;
    END_IF
END_FUNCTION

PROGRAM Main
VAR
    topic : STRING(63) := 'abc';
    i : INT := -1;
    u : UINT := 65535;
    rI : REFERENCE TO INT;
    pastLen : BOOL := TRUE;
    pastShort : BOOL;
    refEqU : BOOL := TRUE;
    refNeqU : BOOL;
END_VAR
rI REF= i;
pastLen := FCheck(sTopic := topic);
pastShort := LEN(topic) > uiShort;
refEqU := rI = u;
refNeqU := rI <> u;
END_PROGRAM
"#;

// Worked by hand: 'abc' has 3 characters, so 3 > 150 is FALSE and 3 > 2 is TRUE; the reference
// names -1, so `rI = u` is FALSE and `rI <> u` TRUE. `pastLen` and `refEqU` start TRUE, so a call or
// a comparison that wrote nothing would leave them TRUE. Catches the build refusing the report's
// `IF LEN(sTopic) > uiLen`, and the reference compared by its bits.
#[test]
fn the_reported_length_check_against_an_unsigned_constant_compiles_under_codesys() {
    let harness = codesys_run(REPORTED, 1);
    for (name, expected) in [
        ("pastLen", false),
        ("pastShort", true),
        ("refEqU", false),
        ("refNeqU", true),
    ] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::Bool(expected)),
            "{name}"
        );
    }
}
