//! Built-in conversion categories independent of compiler type IDs.

/// Recognized IEC built-in names used by native conversion signatures.
/// Generic categories retain recognition and are rejected by conversion policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionType {
    /// IEC `BOOL` conversion category.
    Bool,
    /// IEC `SINT` conversion category.
    SInt,
    /// IEC `INT` conversion category.
    Int,
    /// IEC `DINT` conversion category.
    DInt,
    /// IEC `LINT` conversion category.
    LInt,
    /// IEC `USINT` conversion category.
    USInt,
    /// IEC `UINT` conversion category.
    UInt,
    /// IEC `UDINT` conversion category.
    UDInt,
    /// IEC `ULINT` conversion category.
    ULInt,
    /// IEC `REAL` conversion category.
    Real,
    /// IEC `LREAL` conversion category.
    LReal,
    /// IEC `BYTE` conversion category.
    Byte,
    /// IEC `WORD` conversion category.
    Word,
    /// IEC `DWORD` conversion category.
    DWord,
    /// IEC `LWORD` conversion category.
    LWord,
    /// IEC `TIME` conversion category.
    Time,
    /// IEC `LTIME` conversion category.
    LTime,
    /// IEC `DATE` conversion category.
    Date,
    /// IEC `LDATE` conversion category.
    LDate,
    /// IEC `TIME_OF_DAY` conversion category.
    Tod,
    /// IEC `LTIME_OF_DAY` conversion category.
    LTod,
    /// IEC `DATE_AND_TIME` conversion category.
    Dt,
    /// IEC `LDATE_AND_TIME` conversion category.
    Ldt,
    /// IEC `ANY` conversion category.
    Any,
    /// IEC `ANY_DERIVED` conversion category.
    AnyDerived,
    /// IEC `ANY_ELEMENTARY` conversion category.
    AnyElementary,
    /// IEC `ANY_MAGNITUDE` conversion category.
    AnyMagnitude,
    /// IEC `ANY_INT` conversion category.
    AnyInt,
    /// IEC `ANY_UNSIGNED` conversion category.
    AnyUnsigned,
    /// IEC `ANY_SIGNED` conversion category.
    AnySigned,
    /// IEC `ANY_REAL` conversion category.
    AnyReal,
    /// IEC `ANY_NUM` conversion category.
    AnyNum,
    /// IEC `ANY_DURATION` conversion category.
    AnyDuration,
    /// IEC `ANY_BIT` conversion category.
    AnyBit,
    /// IEC `ANY_CHARS` conversion category.
    AnyChars,
    /// IEC `ANY_STRING` conversion category.
    AnyString,
    /// IEC `ANY_CHAR` conversion category.
    AnyChar,
    /// IEC `ANY_DATE` conversion category.
    AnyDate,
    /// IEC `STRING` conversion category.
    String,
    /// IEC `WSTRING` conversion category.
    WString,
    /// IEC `CHAR` conversion category.
    Char,
    /// IEC `WCHAR` conversion category.
    WChar,
}

impl ConversionType {
    /// Resolve the existing case-insensitive built-in conversion spelling.
    pub fn from_builtin_name(name: &str) -> Option<Self> {
        match name.to_ascii_uppercase().as_str() {
            "BOOL" => Some(Self::Bool),
            "SINT" => Some(Self::SInt),
            "INT" => Some(Self::Int),
            "DINT" => Some(Self::DInt),
            "LINT" => Some(Self::LInt),
            "USINT" => Some(Self::USInt),
            "UINT" => Some(Self::UInt),
            "UDINT" => Some(Self::UDInt),
            "ULINT" => Some(Self::ULInt),
            "REAL" => Some(Self::Real),
            "LREAL" => Some(Self::LReal),
            "BYTE" => Some(Self::Byte),
            "WORD" => Some(Self::Word),
            "DWORD" => Some(Self::DWord),
            "LWORD" => Some(Self::LWord),
            "TIME" => Some(Self::Time),
            "LTIME" => Some(Self::LTime),
            "DATE" => Some(Self::Date),
            "LDATE" => Some(Self::LDate),
            "TOD" | "TIME_OF_DAY" => Some(Self::Tod),
            "LTOD" | "LTIME_OF_DAY" => Some(Self::LTod),
            "DT" | "DATE_AND_TIME" => Some(Self::Dt),
            "LDT" | "LDATE_AND_TIME" => Some(Self::Ldt),
            "ANY" => Some(Self::Any),
            "ANY_DERIVED" => Some(Self::AnyDerived),
            "ANY_ELEMENTARY" => Some(Self::AnyElementary),
            "ANY_MAGNITUDE" => Some(Self::AnyMagnitude),
            "ANY_INT" => Some(Self::AnyInt),
            "ANY_UNSIGNED" => Some(Self::AnyUnsigned),
            "ANY_SIGNED" => Some(Self::AnySigned),
            "ANY_REAL" => Some(Self::AnyReal),
            "ANY_NUM" => Some(Self::AnyNum),
            "ANY_DURATION" => Some(Self::AnyDuration),
            "ANY_BIT" => Some(Self::AnyBit),
            "ANY_CHARS" => Some(Self::AnyChars),
            "ANY_STRING" => Some(Self::AnyString),
            "ANY_CHAR" => Some(Self::AnyChar),
            "ANY_DATE" => Some(Self::AnyDate),
            "STRING" => Some(Self::String),
            "WSTRING" => Some(Self::WString),
            "CHAR" => Some(Self::Char),
            "WCHAR" => Some(Self::WChar),
            _ => None,
        }
    }
}
