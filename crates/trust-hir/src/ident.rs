//! Identifier validation helpers for IEC 61131-3 Structured Text.

/// Returns true if the identifier follows IEC 61131-3 rules.
#[must_use]
pub fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let bytes = name.as_bytes();
    let first = bytes[0];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    if *bytes.last().unwrap() == b'_' {
        return false;
    }

    let mut prev_underscore = false;
    for &b in bytes {
        if !(b.is_ascii_alphanumeric() || b == b'_') {
            return false;
        }
        if b == b'_' {
            if prev_underscore {
                return false;
            }
            prev_underscore = true;
        } else {
            prev_underscore = false;
        }
    }

    true
}

/// Returns true if the identifier is a reserved keyword.
#[must_use]
pub fn is_reserved_keyword(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "PROGRAM"
            | "END_PROGRAM"
            | "FUNCTION"
            | "END_FUNCTION"
            | "FUNCTION_BLOCK"
            | "END_FUNCTION_BLOCK"
            | "CLASS"
            | "END_CLASS"
            | "METHOD"
            | "END_METHOD"
            | "PROPERTY"
            | "END_PROPERTY"
            | "INTERFACE"
            | "END_INTERFACE"
            | "NAMESPACE"
            | "END_NAMESPACE"
            | "USING"
            | "ACTION"
            | "END_ACTION"
            | "VAR"
            | "END_VAR"
            | "VAR_INPUT"
            | "VAR_OUTPUT"
            | "VAR_IN_OUT"
            | "VAR_TEMP"
            | "VAR_GLOBAL"
            | "VAR_EXTERNAL"
            | "VAR_ACCESS"
            | "VAR_CONFIG"
            | "VAR_STAT"
            | "CONSTANT"
            | "RETAIN"
            | "NON_RETAIN"
            | "PERSISTENT"
            | "PUBLIC"
            | "PRIVATE"
            | "PROTECTED"
            | "INTERNAL"
            | "FINAL"
            | "ABSTRACT"
            | "OVERRIDE"
            | "TYPE"
            | "END_TYPE"
            | "STRUCT"
            | "END_STRUCT"
            | "UNION"
            | "END_UNION"
            | "ARRAY"
            | "OF"
            | "STRING"
            | "WSTRING"
            | "POINTER"
            | "REF"
            | "REF_TO"
            | "TO"
            | "EXTENDS"
            | "IMPLEMENTS"
            | "THIS"
            | "SUPER"
            | "NEW"
            | "__NEW"
            | "__DELETE"
            | "IF"
            | "THEN"
            | "ELSIF"
            | "ELSE"
            | "END_IF"
            | "CASE"
            | "END_CASE"
            | "FOR"
            | "END_FOR"
            | "BY"
            | "DO"
            | "WHILE"
            | "END_WHILE"
            | "REPEAT"
            | "UNTIL"
            | "END_REPEAT"
            | "RETURN"
            | "EXIT"
            | "CONTINUE"
            | "JMP"
            | "AND"
            | "OR"
            | "XOR"
            | "NOT"
            | "MOD"
            | "BOOL"
            | "SINT"
            | "INT"
            | "DINT"
            | "LINT"
            | "USINT"
            | "UINT"
            | "UDINT"
            | "ULINT"
            | "REAL"
            | "LREAL"
            | "BYTE"
            | "WORD"
            | "DWORD"
            | "LWORD"
            | "TIME"
            | "LTIME"
            | "DATE"
            | "LDATE"
            | "TIME_OF_DAY"
            | "TOD"
            | "LTIME_OF_DAY"
            | "LTOD"
            | "DATE_AND_TIME"
            | "DT"
            | "LDATE_AND_TIME"
            | "LDT"
            | "CHAR"
            | "WCHAR"
            | "ANY"
            | "ANY_DERIVED"
            | "ANY_ELEMENTARY"
            | "ANY_MAGNITUDE"
            | "ANY_INT"
            | "ANY_UNSIGNED"
            | "ANY_SIGNED"
            | "ANY_REAL"
            | "ANY_NUM"
            | "ANY_DURATION"
            | "ANY_BIT"
            | "ANY_CHARS"
            | "ANY_STRING"
            | "ANY_CHAR"
            | "ANY_DATE"
            | "TRUE"
            | "FALSE"
            | "NULL"
            | "CONFIGURATION"
            | "END_CONFIGURATION"
            | "RESOURCE"
            | "END_RESOURCE"
            | "ON"
            | "TASK"
            | "WITH"
            | "AT"
            | "EN"
            | "ENO"
            | "R_EDGE"
            | "F_EDGE"
            | "ADR"
            | "SIZEOF"
            | "READ_ONLY"
            | "READ_WRITE"
            | "GET"
            | "END_GET"
            | "SET"
            | "END_SET"
            | "STEP"
            | "END_STEP"
            | "INITIAL_STEP"
            | "TRANSITION"
            | "END_TRANSITION"
            | "FROM"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // IEC 61131-3 Ed.3 Table 2 (identifier syntax)
    fn test_valid_identifiers() {
        assert!(is_valid_identifier("A"));
        assert!(is_valid_identifier("_A"));
        assert!(is_valid_identifier("A_B"));
        assert!(is_valid_identifier("ABC123"));
        assert!(is_valid_identifier("A_B_C"));
    }

    #[test]
    // IEC 61131-3 Ed.3 Table 2 (identifier syntax)
    fn test_invalid_identifiers() {
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("1ABC"));
        assert!(!is_valid_identifier("A__B"));
        assert!(!is_valid_identifier("__ABC"));
        assert!(!is_valid_identifier("ABC_"));
        assert!(!is_valid_identifier("_"));
    }

    #[test]
    fn test_reserved_keywords() {
        assert!(is_reserved_keyword("PROGRAM"));
        assert!(is_reserved_keyword("var_input"));
        assert!(is_reserved_keyword("TIME_OF_DAY"));
        assert!(is_reserved_keyword("TOD"));
        assert!(is_reserved_keyword("DT"));
        assert!(is_reserved_keyword("LDT"));
        assert!(is_reserved_keyword("LTOD"));
        assert!(is_reserved_keyword("__delete"));
        assert!(is_reserved_keyword("CLASS"));
        assert!(is_reserved_keyword("CONFIGURATION"));
        assert!(is_reserved_keyword("NULL"));
        assert!(is_reserved_keyword("CHAR"));
        assert!(is_reserved_keyword("WCHAR"));
        assert!(is_reserved_keyword("LDATE"));
        assert!(is_reserved_keyword("ANY_ELEMENTARY"));
        assert!(is_reserved_keyword("ANY_UNSIGNED"));
        assert!(is_reserved_keyword("ANY_DURATION"));
        assert!(is_reserved_keyword("READ_ONLY"));
        assert!(is_reserved_keyword("READ_WRITE"));
        assert!(is_reserved_keyword("EN"));
        assert!(is_reserved_keyword("ENO"));
        assert!(is_reserved_keyword("STEP"));
        assert!(is_reserved_keyword("TRANSITION"));
        assert!(is_reserved_keyword("R_EDGE"));
        assert!(is_reserved_keyword("F_EDGE"));
    }
}

#[cfg(test)]
#[path = "ident/contract_tests.rs"]
mod contract_tests;
