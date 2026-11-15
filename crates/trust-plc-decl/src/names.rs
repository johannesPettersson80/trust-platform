//! Which names a Rust POU's declarations take (IEC §6.1.2; specification 01 §3): an IEC identifier
//! that is no reserved word, so that the declarations parse and check under every profile. The
//! attribute refuses another name where the Rust name stands (specification 48 RU-22).

/// The reserved words of Structured Text as truST's lexer reads them (specification 01 §3), in any
/// case; trust-runtime's tests keep the list equal to the lexer's and to what its parser takes.
pub const RESERVED: [&str; 173] = [
    "ABSTRACT",
    "ACTION",
    "ADR",
    "AND",
    "ANY",
    "ANY_BIT",
    "ANY_CHAR",
    "ANY_CHARS",
    "ANY_DATE",
    "ANY_DERIVED",
    "ANY_DURATION",
    "ANY_ELEMENTARY",
    "ANY_INT",
    "ANY_MAGNITUDE",
    "ANY_NUM",
    "ANY_REAL",
    "ANY_SIGNED",
    "ANY_STRING",
    "ANY_UNSIGNED",
    "ARRAY",
    "AT",
    "BOOL",
    "BY",
    "BYTE",
    "CASE",
    "CHAR",
    "CLASS",
    "CONFIGURATION",
    "CONSTANT",
    "CONTINUE",
    "DATE",
    "DATE_AND_TIME",
    "DINT",
    "DO",
    "DT",
    "DWORD",
    "ELSE",
    "ELSIF",
    "EN",
    "END_ACTION",
    "END_CASE",
    "END_CLASS",
    "END_CONFIGURATION",
    "END_FOR",
    "END_FUNCTION",
    "END_FUNCTION_BLOCK",
    "END_GET",
    "END_IF",
    "END_INTERFACE",
    "END_METHOD",
    "END_NAMESPACE",
    "END_PROGRAM",
    "END_PROPERTY",
    "END_REPEAT",
    "END_RESOURCE",
    "END_SET",
    "END_STEP",
    "END_STRUCT",
    "END_TEST_FUNCTION_BLOCK",
    "END_TEST_PROGRAM",
    "END_TRANSITION",
    "END_TYPE",
    "END_UNION",
    "END_VAR",
    "END_WHILE",
    "ENO",
    "EXIT",
    "EXTENDS",
    "FALSE",
    "FINAL",
    "FOR",
    "FROM",
    "FUNCTION",
    "FUNCTION_BLOCK",
    "F_EDGE",
    "GET",
    "IF",
    "IMPLEMENTS",
    "INITIAL_STEP",
    "INT",
    "INTERFACE",
    "INTERNAL",
    "JMP",
    "LDATE",
    "LDATE_AND_TIME",
    "LDT",
    "LINT",
    "LREAL",
    "LTIME",
    "LTIME_OF_DAY",
    "LTOD",
    "LWORD",
    "METHOD",
    "MOD",
    "NAMESPACE",
    "NEW",
    "NON_RETAIN",
    "NOT",
    "NULL",
    "OF",
    "ON",
    "OR",
    "OVERLAP",
    "OVERRIDE",
    "PERSISTENT",
    "POINTER",
    "PRIVATE",
    "PROGRAM",
    "PROPERTY",
    "PROTECTED",
    "PUBLIC",
    "READ_ONLY",
    "READ_WRITE",
    "REAL",
    "REF",
    "REF_TO",
    "REPEAT",
    "RESOURCE",
    "RETAIN",
    "RETURN",
    "R_EDGE",
    "SET",
    "SINT",
    "SIZEOF",
    "STEP",
    "STRING",
    "STRUCT",
    "SUPER",
    "TASK",
    "TEST_FUNCTION_BLOCK",
    "TEST_PROGRAM",
    "THEN",
    "THIS",
    "TIME",
    "TIME_OF_DAY",
    "TO",
    "TOD",
    "TRANSITION",
    "TRUE",
    "TYPE",
    "UDINT",
    "UINT",
    "ULINT",
    "UNION",
    "UNTIL",
    "USING",
    "USINT",
    "VAR",
    "VAR_ACCESS",
    "VAR_CONFIG",
    "VAR_EXTERNAL",
    "VAR_GLOBAL",
    "VAR_INPUT",
    "VAR_INST",
    "VAR_IN_OUT",
    "VAR_OUTPUT",
    "VAR_STAT",
    "VAR_TEMP",
    "WCHAR",
    "WHILE",
    "WITH",
    "WORD",
    "WSTRING",
    "XOR",
    "__CATCH",
    "__DELETE",
    "__ENDTRY",
    "__FINALLY",
    "__ISVALIDREF",
    "__NEW",
    "__QUERYINTERFACE",
    "__SYSTEM",
    "__TRY",
];

/// The reserved words that only the CODESYS profile takes as a name of a variable, a parameter or
/// an enumeration value (specification 01 §3; `docs/IEC_DEVIATIONS.md` DEV-034).
const CODESYS_NAMES: [&str; 3] = ["NULL", "ON", "SET"];

/// Whether `name` is one of Structured Text's reserved words, in any case.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    RESERVED.iter().any(|word| word.eq_ignore_ascii_case(name))
}

/// Whether a Rust POU's declarations may name anything `name`: a POU, a type, a namespace, a
/// variable, a field, a parameter or an enumeration value. It is an IEC identifier and no reserved
/// word, in every profile: not `NULL`, `ON` or `SET` either, which only the CODESYS profile takes.
#[must_use]
pub fn takes_name(name: &str) -> bool {
    crate::is_identifier(name) && !is_reserved(name)
}

/// The refusal's reason for `name`, a reserved word: why it names nothing, and the CODESYS
/// profile's names said so.
pub(crate) fn reserved_reason(name: &str) -> String {
    let codesys = CODESYS_NAMES
        .iter()
        .any(|word| word.eq_ignore_ascii_case(name));
    format!(
        "`{name}` is a reserved word of Structured Text (specification 01 §3){}",
        if codesys {
            ", which only the CODESYS profile takes as a name (docs/IEC_DEVIATIONS.md DEV-034); \
             a Rust POU's declarations hold under every profile"
        } else {
            ""
        }
    )
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
