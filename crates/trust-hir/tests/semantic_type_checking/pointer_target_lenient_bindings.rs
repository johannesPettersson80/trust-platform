use crate::common::*;

/// Diagnostics of `source` analyzed under the CODESYS profile as
/// (code, severity) pairs — the lenient pointer bindings are a codesys
/// departure (IEC_DEVIATIONS 2026-10-29, DEV-039).
fn binding_diagnostics(source: &str) -> Vec<(DiagnosticCode, DiagnosticSeverity)> {
    diagnostics_under(Dialect::Codesys, source)
}

/// Diagnostics of `source` analyzed under `dialect` as (code, severity) pairs.
fn diagnostics_under(dialect: Dialect, source: &str) -> Vec<(DiagnosticCode, DiagnosticSeverity)> {
    let mut db = Database::new();
    db.set_dialect(dialect);
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .map(|d| (d.code, d.severity))
        .collect()
}

/// Codesys-profile acceptance: zero errors and exactly one `<code>` warning.
fn assert_single_lenient_warning(source: &str, code: DiagnosticCode) {
    let diagnostics = binding_diagnostics(source);
    let errors = diagnostics
        .iter()
        .filter(|(_, severity)| *severity == DiagnosticSeverity::Error)
        .count();
    assert_eq!(
        errors, 0,
        "lenient binding must compile without errors: {diagnostics:?}"
    );
    let warnings = diagnostics
        .iter()
        .filter(|(c, severity)| *c == code && *severity == DiagnosticSeverity::Warning)
        .count();
    assert_eq!(
        warnings, 1,
        "expected exactly one {code:?} warning: {diagnostics:?}"
    );
}

/// Codesys-profile silence: no error, and no diagnostic of either pointer binding code.
fn assert_silent(source: &str) {
    let diagnostics = binding_diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .all(|(code, severity)| *severity != DiagnosticSeverity::Error
                && *code != DiagnosticCode::InvalidArgumentType
                && *code != DiagnosticCode::IncompatibleAssignment),
        "an ADR-sourced lenient binding must be silent: {diagnostics:?}"
    );
}

/// Codesys-profile rejection: exactly one `<code>` error and no warning of that code.
fn assert_single_strict_error(source: &str, code: DiagnosticCode) {
    let diagnostics = binding_diagnostics(source);
    let errors = diagnostics
        .iter()
        .filter(|(c, severity)| *c == code && *severity == DiagnosticSeverity::Error)
        .count();
    assert_eq!(
        errors, 1,
        "expected exactly one {code:?} error: {diagnostics:?}"
    );
    let warnings = diagnostics
        .iter()
        .filter(|(c, severity)| *c == code && *severity == DiagnosticSeverity::Warning)
        .count();
    assert_eq!(
        warnings, 0,
        "non-lenient binding must not demote: {diagnostics:?}"
    );
}

#[test]
fn char_pointer_source_binds_byte_parameter_as_warning() {
    // CODESYS pointer-compatibility parity (IEC_DEVIATIONS 2026-10-29, DEV-039):
    // a `POINTER TO CHAR` variable binds a `POINTER TO BYTE` formal — the
    // vendor MemCopy idiom — as a warning: its source is a pointer value, not
    // an `ADR` call, so the reinterpretation stays visible. Catches a pointer
    // variable that inherits the silence of the `ADR` it was assigned from.
    assert_single_lenient_warning(
        r#"
FUNCTION PtrSink : DINT
VAR_INPUT
    pDest : POINTER TO BYTE;
    pSource : POINTER TO BYTE;
END_VAR
PtrSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    aBuf : ARRAY[0..15] OF BYTE;
    sJson : STRING[32];
    pChar : POINTER TO CHAR;
END_VAR
pChar := ADR(sJson[1]);
PtrSink(pDest := ADR(aBuf[0]), pSource := pChar);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn string_pointer_binds_byte_array_pointer_parameter_as_warning() {
    // The vendor Fb_JSON_ParseAndModify idiom with a pointer variable: a
    // `POINTER TO STRING[2048]` binds a `POINTER TO ARRAY[0..65535] OF BYTE`
    // formal as a warning ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
    assert_single_lenient_warning(
        r#"
FUNCTION JsonSink : DINT
VAR_INPUT
    pData : POINTER TO ARRAY[0..65535] OF BYTE;
    udiSizeData : UDINT;
END_VAR
JsonSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    sJson : STRING[2048];
    pJson : POINTER TO STRING[2048];
END_VAR
pJson := ADR(sJson);
JsonSink(pData := pJson, udiSizeData := 16);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn byte_pointer_argument_binds_char_parameter_as_warning() {
    // The 1-byte leniency is symmetric: a byte-pointer variable binds a
    // CHAR-pointer formal with a warning ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
    assert_single_lenient_warning(
        r#"
FUNCTION CharSink : DINT
VAR_INPUT
    pChar : POINTER TO CHAR;
END_VAR
CharSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    aB : ARRAY[0..7] OF BYTE;
    pB : POINTER TO BYTE;
END_VAR
pB := ADR(aB[0]);
CharSink(pChar := pB);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn char_pointer_to_byte_target_assignment_warns() {
    // The leniency also covers plain assignments (E203 demotion): a typed
    // byte pointer takes a CHAR-pointer variable with a warning
    // ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
    assert_single_lenient_warning(
        r#"
PROGRAM Test
VAR
    s : STRING[16];
    pC : POINTER TO CHAR;
    pB : POINTER TO BYTE;
END_VAR
pC := ADR(s[1]);
pB := pC;
END_PROGRAM
"#,
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn int_pointer_to_byte_pointer_parameter_stays_error() {
    // The record boundary stays strict: POINTER TO INT does not bind a
    // POINTER TO BYTE formal (IEC_DEVIATIONS 2026-10-29, DEV-039 — form-scoped, not blanket).
    assert_single_strict_error(
        r#"
FUNCTION IntSink : DINT
VAR_INPUT
    pInt : POINTER TO INT;
END_VAR
IntSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    aB : ARRAY[0..7] OF BYTE;
END_VAR
IntSink(pInt := ADR(aB[0]));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn string_to_int_array_pointer_parameter_stays_error() {
    // `ADR(STRING)` only binds byte-array pointers; an ARRAY OF INT
    // formal keeps the error ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
    assert_single_strict_error(
        r#"
FUNCTION IntArrSink : DINT
VAR_INPUT
    pData : POINTER TO ARRAY[0..9] OF INT;
END_VAR
IntArrSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    s : STRING[8];
END_VAR
IntArrSink(pData := ADR(s));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn string_pointer_binds_byte_pointer_parameter_as_warning() {
    // The vendor MemCopy idiom at the string base with a pointer variable: a
    // `POINTER TO STRING[n]` binds a plain `POINTER TO BYTE` formal as a
    // warning (DEV-039).
    assert_single_lenient_warning(
        r#"
FUNCTION MemCopy : DINT
VAR_INPUT
    pDest : POINTER TO BYTE;
    pSource : POINTER TO BYTE;
    udiSize : UDINT;
END_VAR
MemCopy := 0;
END_FUNCTION

PROGRAM Test
VAR
    aBuf : ARRAY[0..15] OF BYTE;
    sPayload : STRING[64];
    pPayload : POINTER TO STRING[64];
END_VAR
pPayload := ADR(sPayload);
MemCopy(pDest := ADR(aBuf[0]), pSource := pPayload, udiSize := 8);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn string_pointer_to_byte_pointer_target_assignment_warns() {
    // The assignment form of the same idiom: a typed byte pointer takes a
    // `POINTER TO STRING[n]` variable with a warning, not an error
    // ((IEC_DEVIATIONS, DEV-039 byte-buffer forms).
    assert_single_lenient_warning(
        r#"
PROGRAM Test
VAR
    s : STRING[16];
    pS : POINTER TO STRING[16];
    pB : POINTER TO BYTE;
END_VAR
pS := ADR(s);
pB := pS;
END_PROGRAM
"#,
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn int_array_element_adr_to_byte_pointer_parameter_stays_error() {
    // The byte-buffer forms are string-scoped: `ADR` of a wider element
    // (ARRAY OF INT) does not bind a POINTER TO BYTE formal
    // (IEC_DEVIATIONS, DEV-039 — form-scoped, not blanket).
    assert_single_strict_error(
        r#"
FUNCTION ByteSink : DINT
VAR_INPUT
    pSource : POINTER TO BYTE;
END_VAR
ByteSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    aInt : ARRAY[0..7] OF INT;
END_VAR
ByteSink(pSource := ADR(aInt[0]));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn adr_sources_bind_the_lenient_formals_silently() {
    // DEV-039 round 2 (#295): under the CODESYS profile a binding whose source
    // is an explicit `ADR(...)` call is silent, as the CODESYS compiler binds
    // it: the base address and an element alike, against a `POINTER TO BYTE`,
    // a `POINTER TO ARRAY[i..j] OF BYTE` and a `POINTER TO CHAR` formal, named
    // and positional, of a function and of a function block, inside any
    // parentheses. Catches an `ADR` source that still warns, and a check that
    // looks at the source's type instead of the call.
    assert_silent(
        r#"
FUNCTION MemCopy : DINT
VAR_INPUT
    pDest : POINTER TO BYTE;
    pSource : POINTER TO BYTE;
    udiSize : UDINT;
END_VAR
MemCopy := 0;
END_FUNCTION

FUNCTION CharSink : DINT
VAR_INPUT
    pChar : POINTER TO CHAR;
END_VAR
CharSink := 0;
END_FUNCTION

FUNCTION_BLOCK FB_JsonParser
VAR_INPUT
    pData : POINTER TO ARRAY[0..65535] OF BYTE;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Test
VAR
    aBuf : ARRAY[0..15] OF BYTE;
    sPayload : STRING[64];
    sJson : STRING[2048];
    fbJsonParser : FB_JsonParser;
END_VAR
MemCopy(pDest := ADR(aBuf[0]), pSource := ADR(sPayload), udiSize := 8);
MemCopy(pDest := ADR(sJson[1]), pSource := ADR(aBuf[0]), udiSize := 8);
fbJsonParser(pData := ADR(sJson));
CharSink(ADR(aBuf[0]));
MemCopy(pDest := (ADR(sPayload[1])), pSource := ((ADR(sPayload))), udiSize := 8);
END_PROGRAM
"#,
    );
}

#[test]
fn adr_sources_assign_the_lenient_targets_silently() {
    // The assignment form of DEV-039 round 2 (#295): a typed byte pointer, a
    // byte-buffer pointer and a CHAR pointer take an explicit `ADR(...)`
    // silently. Catches the assignment check left warning while calls go
    // silent.
    assert_silent(
        r#"
PROGRAM Test
VAR
    s : STRING[16];
    aB : ARRAY[0..7] OF BYTE;
    pB : POINTER TO BYTE;
    pBuf : POINTER TO ARRAY[0..15] OF BYTE;
    pC : POINTER TO CHAR;
END_VAR
pB := ADR(s);
pB := ADR(s[1]);
pBuf := ADR(s);
pC := (ADR(aB[0]));
END_PROGRAM
"#,
    );
}

#[test]
fn an_adr_element_against_a_byte_array_pointer_stays_error() {
    // An `ADR` source silences only the bindings DEV-039 makes lenient: a
    // string element is a `POINTER TO CHAR`, which no `POINTER TO ARRAY OF
    // BYTE` formal takes. Catches an `ADR` source that widens the leniency.
    assert_single_strict_error(
        r#"
FUNCTION JsonSink : DINT
VAR_INPUT
    pData : POINTER TO ARRAY[0..65535] OF BYTE;
END_VAR
JsonSink := 0;
END_FUNCTION

PROGRAM Test
VAR
    sJson : STRING[2048];
END_VAR
JsonSink(pData := ADR(sJson[1]));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn no_pou_can_take_the_name_adr() {
    // The ruling keeps the warning for a user POU that shadows `ADR`. In truST
    // `ADR` is a keyword, so such a POU cannot be declared and every `ADR(...)`
    // is the address-of operator. Catches a grammar that admits `ADR` as a POU
    // name, which would let a user function pass for the operator.
    let source = r#"
FUNCTION ADR : DINT
VAR_INPUT
    x : DINT;
END_VAR
ADR := x;
END_FUNCTION
"#;
    let diagnostics = binding_diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|(_, severity)| *severity == DiagnosticSeverity::Error),
        "a POU named ADR must be refused: {diagnostics:?}"
    );
    let renamed = binding_diagnostics(&source.replace("ADR", "AdrOf"));
    assert!(
        renamed
            .iter()
            .all(|(_, severity)| *severity != DiagnosticSeverity::Error),
        "the same function under another name is valid, so the name is refused: {renamed:?}"
    );
}

#[test]
fn the_iec_profile_keeps_the_adr_bindings_errors() {
    // DEV-039 is a CODESYS departure: under the IEC profile the same `ADR`
    // bindings stay errors, one for the call and one for the assignment.
    // Catches the `ADR` silence leaking out of the CODESYS profile.
    let diagnostics = diagnostics_under(
        Dialect::Iec,
        r#"
FUNCTION MemCopy : DINT
VAR_INPUT
    pSource : POINTER TO BYTE;
END_VAR
MemCopy := 0;
END_FUNCTION

PROGRAM Test
VAR
    sPayload : STRING[64];
    pB : POINTER TO BYTE;
END_VAR
MemCopy(pSource := ADR(sPayload));
pB := ADR(sPayload[1]);
END_PROGRAM
"#,
    );
    for code in [
        DiagnosticCode::InvalidArgumentType,
        DiagnosticCode::IncompatibleAssignment,
    ] {
        assert_eq!(
            diagnostics
                .iter()
                .filter(|(c, severity)| *c == code && *severity == DiagnosticSeverity::Error)
                .count(),
            1,
            "{code:?}: {diagnostics:?}"
        );
    }
}
