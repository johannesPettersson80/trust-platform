use crate::common::*;

/// Diagnostics of `source` analyzed under the CODESYS profile as
/// (code, severity) pairs — the lenient pointer bindings are a codesys
/// departure (IEC_DEVIATIONS 2026-10-29, DEV-039).
fn binding_diagnostics(source: &str) -> Vec<(DiagnosticCode, DiagnosticSeverity)> {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
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
    // `ADR(str[1])` keeps its `POINTER TO CHAR` typing and still binds a
    // `POINTER TO BYTE` formal — the vendor MemCopy idiom — as a warning.
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
END_VAR
PtrSink(pDest := ADR(aBuf[0]), pSource := ADR(sJson[1]));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn string_adr_binds_byte_array_pointer_parameter_as_warning() {
    // The vendor Fb_JSON_ParseAndModify idiom: `ADR(STRING[2048])` binds
    // a `POINTER TO ARRAY[0..65535] OF BYTE` formal as a warning
    // ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
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
END_VAR
JsonSink(pData := ADR(sJson), udiSizeData := 16);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn byte_pointer_argument_binds_char_parameter_as_warning() {
    // The 1-byte leniency is symmetric: a byte-pointer argument binds a
    // CHAR-pointer formal ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
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
END_VAR
CharSink(pChar := ADR(aB[0]));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn char_pointer_to_byte_target_assignment_warns() {
    // The leniency also covers plain assignments (E203 demotion), so a
    // typed byte pointer can take `ADR` of a string element ((IEC_DEVIATIONS 2026-10-29, DEV-039)).
    assert_single_lenient_warning(
        r#"
PROGRAM Test
VAR
    s : STRING[16];
    pB : POINTER TO BYTE;
END_VAR
pB := ADR(s[1]);
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
fn string_adr_binds_byte_pointer_parameter_as_warning() {
    // The vendor MemCopy idiom at the string base: `ADR(STRING[n])` binds
    // a plain `POINTER TO BYTE` formal as a warning (DEV-039; the vendor
    // copy loop binds the string's base address — the element form
    // `ADR(s[1])` would copy from offset one and lose the first byte).
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
END_VAR
MemCopy(pDest := ADR(aBuf[0]), pSource := ADR(sPayload), udiSize := 8);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn string_base_adr_to_byte_pointer_target_assignment_warns() {
    // The assignment form of the same idiom: a typed byte pointer takes
    // `ADR` of the string base ((IEC_DEVIATIONS, DEV-039 byte-buffer
    // forms), diagnosed as a warning, not an error.
    assert_single_lenient_warning(
        r#"
PROGRAM Test
VAR
    s : STRING[16];
    pB : POINTER TO BYTE;
END_VAR
pB := ADR(s);
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
