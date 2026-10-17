//! Declared lengths of `STRING` and `WSTRING` (IEC 61131-3 Ed.3 §6.4.2.2 and Table 10 note k;
//! `docs/IEC_DECISIONS.md` 2026-10-17).
//!
//! A `STRING[n]` holds up to n bytes of UTF-8 and a `WSTRING[n]` up to n UTF-16 code units. A
//! declaration without a length is `STRING[80]` or `WSTRING[80]`, as in TwinCAT 3 and CODESYS.

/// The length of a `STRING` or `WSTRING` declared without one.
pub const DEFAULT_STRING_LENGTH: u32 = 80;

/// The implementer-specific maximum length of IEC 61131-3 Ed.3 §6.4.2.2: the longest declared
/// length, and the longest text a string function may produce.
pub const MAX_STRING_LENGTH: u32 = 65_535;

/// Why a declared `STRING[n]` or `WSTRING[n]` length is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringLengthError {
    /// The length is zero or negative.
    NotPositive,
    /// The length is larger than [`MAX_STRING_LENGTH`].
    AboveMaximum,
}

impl StringLengthError {
    /// The diagnostic text for this refusal.
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::NotPositive => "string length must be a positive integer".to_string(),
            Self::AboveMaximum => {
                format!("string length exceeds the maximum of {MAX_STRING_LENGTH}")
            }
        }
    }
}

/// Check the constant value of a declared length against 1 to [`MAX_STRING_LENGTH`].
///
/// # Errors
///
/// Returns why the length is refused.
pub fn checked_string_length(value: i64) -> Result<u32, StringLengthError> {
    if value <= 0 {
        return Err(StringLengthError::NotPositive);
    }
    u32::try_from(value)
        .ok()
        .filter(|length| *length <= MAX_STRING_LENGTH)
        .ok_or(StringLengthError::AboveMaximum)
}

#[cfg(test)]
mod tests {
    use super::{checked_string_length, StringLengthError, MAX_STRING_LENGTH};

    #[test]
    fn declared_lengths_run_from_one_to_the_implementer_maximum() {
        assert_eq!(checked_string_length(1), Ok(1));
        assert_eq!(checked_string_length(65_535), Ok(MAX_STRING_LENGTH));
        assert_eq!(
            checked_string_length(0),
            Err(StringLengthError::NotPositive)
        );
        assert_eq!(
            checked_string_length(-1),
            Err(StringLengthError::NotPositive)
        );
        assert_eq!(
            checked_string_length(65_536),
            Err(StringLengthError::AboveMaximum)
        );
        assert_eq!(
            checked_string_length(i64::MAX),
            Err(StringLengthError::AboveMaximum)
        );
        assert_eq!(
            StringLengthError::AboveMaximum.message(),
            "string length exceeds the maximum of 65535"
        );
    }
}
