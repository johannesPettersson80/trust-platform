//! Type system for IEC 61131-3 Structured Text.
//!
//! This module defines all types in the ST type system, including elementary
//! types, compound types, and user-defined types.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use text_size::TextRange;

/// A unique identifier for a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

impl TypeId {
    // Elementary type IDs (built-in)
    /// Unknown/error type.
    pub const UNKNOWN: Self = Self(0);
    /// Void type (no return value).
    pub const VOID: Self = Self(1);
    /// BOOL type.
    pub const BOOL: Self = Self(2);
    /// SINT type (8-bit signed).
    pub const SINT: Self = Self(3);
    /// INT type (16-bit signed).
    pub const INT: Self = Self(4);
    /// DINT type (32-bit signed).
    pub const DINT: Self = Self(5);
    /// LINT type (64-bit signed).
    pub const LINT: Self = Self(6);
    /// USINT type (8-bit unsigned).
    pub const USINT: Self = Self(7);
    /// UINT type (16-bit unsigned).
    pub const UINT: Self = Self(8);
    /// UDINT type (32-bit unsigned).
    pub const UDINT: Self = Self(9);
    /// ULINT type (64-bit unsigned).
    pub const ULINT: Self = Self(10);
    /// REAL type (32-bit float).
    pub const REAL: Self = Self(11);
    /// LREAL type (64-bit float).
    pub const LREAL: Self = Self(12);
    /// BYTE type (8-bit bit string).
    pub const BYTE: Self = Self(13);
    /// WORD type (16-bit bit string).
    pub const WORD: Self = Self(14);
    /// DWORD type (32-bit bit string).
    pub const DWORD: Self = Self(15);
    /// LWORD type (64-bit bit string).
    pub const LWORD: Self = Self(16);
    /// TIME type.
    pub const TIME: Self = Self(17);
    /// LTIME type.
    pub const LTIME: Self = Self(18);
    /// DATE type.
    pub const DATE: Self = Self(19);
    /// LDATE type.
    pub const LDATE: Self = Self(36);
    /// ANY_DERIVED type.
    pub const ANY_DERIVED: Self = Self(37);
    /// ANY_ELEMENTARY type.
    pub const ANY_ELEMENTARY: Self = Self(38);
    /// ANY_MAGNITUDE type.
    pub const ANY_MAGNITUDE: Self = Self(39);
    /// ANY_UNSIGNED type.
    pub const ANY_UNSIGNED: Self = Self(40);
    /// ANY_SIGNED type.
    pub const ANY_SIGNED: Self = Self(41);
    /// ANY_DURATION type.
    pub const ANY_DURATION: Self = Self(42);
    /// ANY_CHARS type.
    pub const ANY_CHARS: Self = Self(43);
    /// ANY_CHAR type.
    pub const ANY_CHAR: Self = Self(44);
    /// TIME_OF_DAY type.
    pub const TOD: Self = Self(20);
    /// DATE_AND_TIME type.
    pub const DT: Self = Self(21);
    /// STRING type.
    pub const STRING: Self = Self(22);
    /// WSTRING type.
    pub const WSTRING: Self = Self(23);
    /// LTIME_OF_DAY type.
    pub const LTOD: Self = Self(24);
    /// LDATE_AND_TIME type.
    pub const LDT: Self = Self(25);
    /// ANY type.
    pub const ANY: Self = Self(26);
    /// ANY_INT type.
    pub const ANY_INT: Self = Self(27);
    /// ANY_REAL type.
    pub const ANY_REAL: Self = Self(28);
    /// ANY_NUM type.
    pub const ANY_NUM: Self = Self(29);
    /// ANY_BIT type.
    pub const ANY_BIT: Self = Self(30);
    /// ANY_STRING type.
    pub const ANY_STRING: Self = Self(31);
    /// ANY_DATE type.
    pub const ANY_DATE: Self = Self(32);
    /// NULL literal type.
    pub const NULL: Self = Self(33);
    /// CHAR type.
    pub const CHAR: Self = Self(34);
    /// WCHAR type.
    pub const WCHAR: Self = Self(35);

    /// First user-defined type ID.
    pub const USER_TYPES_START: u32 = 100;

    /// Returns a built-in type ID for a given name (case-insensitive).
    #[must_use]
    pub fn from_builtin_name(name: &str) -> Option<Self> {
        match name.to_ascii_uppercase().as_str() {
            "BOOL" => Some(Self::BOOL),
            "SINT" => Some(Self::SINT),
            "INT" => Some(Self::INT),
            "DINT" => Some(Self::DINT),
            "LINT" => Some(Self::LINT),
            "USINT" => Some(Self::USINT),
            "UINT" => Some(Self::UINT),
            "UDINT" => Some(Self::UDINT),
            "ULINT" => Some(Self::ULINT),
            "REAL" => Some(Self::REAL),
            "LREAL" => Some(Self::LREAL),
            "BYTE" => Some(Self::BYTE),
            "WORD" => Some(Self::WORD),
            "DWORD" => Some(Self::DWORD),
            "LWORD" => Some(Self::LWORD),
            "TIME" => Some(Self::TIME),
            "LTIME" => Some(Self::LTIME),
            "DATE" => Some(Self::DATE),
            "LDATE" => Some(Self::LDATE),
            "TOD" | "TIME_OF_DAY" => Some(Self::TOD),
            "LTOD" | "LTIME_OF_DAY" => Some(Self::LTOD),
            "DT" | "DATE_AND_TIME" => Some(Self::DT),
            "LDT" | "LDATE_AND_TIME" => Some(Self::LDT),
            "ANY" => Some(Self::ANY),
            "ANY_DERIVED" => Some(Self::ANY_DERIVED),
            "ANY_ELEMENTARY" => Some(Self::ANY_ELEMENTARY),
            "ANY_MAGNITUDE" => Some(Self::ANY_MAGNITUDE),
            "ANY_INT" => Some(Self::ANY_INT),
            "ANY_UNSIGNED" => Some(Self::ANY_UNSIGNED),
            "ANY_SIGNED" => Some(Self::ANY_SIGNED),
            "ANY_REAL" => Some(Self::ANY_REAL),
            "ANY_NUM" => Some(Self::ANY_NUM),
            "ANY_DURATION" => Some(Self::ANY_DURATION),
            "ANY_BIT" => Some(Self::ANY_BIT),
            "ANY_CHARS" => Some(Self::ANY_CHARS),
            "ANY_STRING" => Some(Self::ANY_STRING),
            "ANY_CHAR" => Some(Self::ANY_CHAR),
            "ANY_DATE" => Some(Self::ANY_DATE),
            "STRING" => Some(Self::STRING),
            "WSTRING" => Some(Self::WSTRING),
            "CHAR" => Some(Self::CHAR),
            "WCHAR" => Some(Self::WCHAR),
            _ => None,
        }
    }

    /// Returns the canonical name for a built-in type ID.
    #[must_use]
    pub fn builtin_name(self) -> Option<&'static str> {
        match self {
            Self::BOOL => Some("BOOL"),
            Self::SINT => Some("SINT"),
            Self::INT => Some("INT"),
            Self::DINT => Some("DINT"),
            Self::LINT => Some("LINT"),
            Self::USINT => Some("USINT"),
            Self::UINT => Some("UINT"),
            Self::UDINT => Some("UDINT"),
            Self::ULINT => Some("ULINT"),
            Self::REAL => Some("REAL"),
            Self::LREAL => Some("LREAL"),
            Self::BYTE => Some("BYTE"),
            Self::WORD => Some("WORD"),
            Self::DWORD => Some("DWORD"),
            Self::LWORD => Some("LWORD"),
            Self::TIME => Some("TIME"),
            Self::LTIME => Some("LTIME"),
            Self::DATE => Some("DATE"),
            Self::LDATE => Some("LDATE"),
            Self::TOD => Some("TIME_OF_DAY"),
            Self::LTOD => Some("LTIME_OF_DAY"),
            Self::DT => Some("DATE_AND_TIME"),
            Self::LDT => Some("LDATE_AND_TIME"),
            Self::ANY => Some("ANY"),
            Self::ANY_DERIVED => Some("ANY_DERIVED"),
            Self::ANY_ELEMENTARY => Some("ANY_ELEMENTARY"),
            Self::ANY_MAGNITUDE => Some("ANY_MAGNITUDE"),
            Self::ANY_INT => Some("ANY_INT"),
            Self::ANY_UNSIGNED => Some("ANY_UNSIGNED"),
            Self::ANY_SIGNED => Some("ANY_SIGNED"),
            Self::ANY_REAL => Some("ANY_REAL"),
            Self::ANY_NUM => Some("ANY_NUM"),
            Self::ANY_DURATION => Some("ANY_DURATION"),
            Self::ANY_BIT => Some("ANY_BIT"),
            Self::ANY_CHARS => Some("ANY_CHARS"),
            Self::ANY_STRING => Some("ANY_STRING"),
            Self::ANY_CHAR => Some("ANY_CHAR"),
            Self::ANY_DATE => Some("ANY_DATE"),
            Self::STRING => Some("STRING"),
            Self::WSTRING => Some("WSTRING"),
            Self::CHAR => Some("CHAR"),
            Self::WCHAR => Some("WCHAR"),
            Self::NULL => Some("NULL"),
            _ => None,
        }
    }
}

/// Lightweight handle for a declaration/default initializer record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InitializerId(pub u32);

/// Source-backed initializer metadata owned by HIR symbol analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializerRecord {
    /// Source range of the initializer expression.
    pub range: TextRange,
}

/// Project-local initializer catalog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InitializerCatalog {
    records: Vec<InitializerRecord>,
    type_defaults: FxHashMap<TypeId, InitializerId>,
}

impl InitializerCatalog {
    /// Inserts a source-backed initializer record and returns a project-local ID.
    pub fn insert(&mut self, record: InitializerRecord) -> InitializerId {
        let id = InitializerId(self.records.len() as u32);
        self.records.push(record);
        id
    }

    /// Returns an initializer record by project-local ID.
    #[must_use]
    pub fn get(&self, id: InitializerId) -> Option<&InitializerRecord> {
        self.records.get(id.0 as usize)
    }

    /// Returns all initializer records in insertion order.
    #[must_use]
    pub fn records(&self) -> &[InitializerRecord] {
        &self.records
    }

    /// Associates a TYPE-level default initializer with the derived type.
    pub fn set_type_default(&mut self, type_id: TypeId, initializer: InitializerId) {
        self.type_defaults.insert(type_id, initializer);
    }

    /// Returns the TYPE-level default initializer for a derived type.
    #[must_use]
    pub fn type_default(&self, type_id: TypeId) -> Option<InitializerId> {
        self.type_defaults.get(&type_id).copied()
    }
}

/// Field definition for structured types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    /// Field name.
    pub name: SmolStr,
    /// Field type identifier.
    pub type_id: TypeId,
    /// Optional direct address (`AT`) for the field.
    pub address: Option<SmolStr>,
    /// Optional default initializer for this field.
    pub default_initializer: Option<InitializerId>,
}

/// Variant definition for union types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionVariant {
    /// Variant name.
    pub name: SmolStr,
    /// Variant type identifier.
    pub type_id: TypeId,
    /// Optional direct address (`AT`) for the variant.
    pub address: Option<SmolStr>,
    /// Optional default initializer for this variant.
    pub default_initializer: Option<InitializerId>,
}

/// Helpers for array dimensions encoded as `(lower, upper)` pairs.
pub trait ArrayDimensionExt {
    /// Returns true when the dimension is the sentinel wildcard form.
    fn is_wildcard(&self) -> bool;

    /// Formats the dimension using `*` for the wildcard sentinel.
    fn display_bounds(&self) -> String;
}

impl ArrayDimensionExt for (i64, i64) {
    fn is_wildcard(&self) -> bool {
        self.0 == 0 && self.1 == i64::MAX
    }

    fn display_bounds(&self) -> String {
        if self.is_wildcard() {
            "*".to_string()
        } else {
            format!("{}..{}", self.0, self.1)
        }
    }
}

/// A type in the ST type system.
// Variant names map directly to standard ST type names.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Unknown/error type.
    Unknown,
    /// Void type (no value).
    Void,
    /// NULL literal type.
    Null,

    // Elementary types
    /// Boolean type.
    Bool,
    /// Signed integer types.
    SInt,
    Int,
    DInt,
    LInt,
    /// Unsigned integer types.
    USInt,
    UInt,
    UDInt,
    ULInt,
    /// Floating point types.
    Real,
    LReal,
    /// Bit string types.
    Byte,
    Word,
    DWord,
    LWord,
    /// Time types.
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    Dt,
    Ldt,
    /// String types.
    String {
        /// Maximum length, if specified.
        max_len: Option<u32>,
    },
    WString {
        /// Maximum length, if specified.
        max_len: Option<u32>,
    },
    /// Character types.
    Char,
    WChar,

    // Compound types
    /// Array type.
    Array {
        /// Element type.
        element: TypeId,
        /// Array dimensions (lower..upper for each dimension).
        dimensions: Vec<(i64, i64)>,
    },
    /// Struct type.
    Struct {
        /// Struct name.
        name: SmolStr,
        /// Field definitions.
        fields: Vec<StructField>,
    },
    /// Union type.
    Union {
        /// Union name.
        name: SmolStr,
        /// Variant definitions.
        variants: Vec<UnionVariant>,
    },
    /// Enum type.
    Enum {
        /// Enum name.
        name: SmolStr,
        /// Base type (default: INT).
        base: TypeId,
        /// Value names and their numeric values.
        values: Vec<(SmolStr, i64)>,
    },
    /// Pointer type.
    Pointer {
        /// Target type.
        target: TypeId,
    },
    /// Reference type.
    Reference {
        /// Target type.
        target: TypeId,
    },
    /// Subrange type (integer-only).
    Subrange {
        /// Base integer type.
        base: TypeId,
        /// Lower bound (inclusive).
        lower: i64,
        /// Upper bound (inclusive).
        upper: i64,
    },

    // User-defined POU types
    /// Function block type.
    FunctionBlock {
        /// Function block name.
        name: SmolStr,
    },
    /// Class type.
    Class {
        /// Class name.
        name: SmolStr,
    },
    /// Interface type.
    Interface {
        /// Interface name.
        name: SmolStr,
    },

    // Generic types (for library functions)
    /// Any type.
    Any,
    /// Any derived type.
    AnyDerived,
    /// Any elementary type.
    AnyElementary,
    /// Any magnitude type.
    AnyMagnitude,
    /// Any integer type.
    AnyInt,
    /// Any unsigned integer type.
    AnyUnsigned,
    /// Any signed integer type.
    AnySigned,
    /// Any real type.
    AnyReal,
    /// Any numeric type.
    AnyNum,
    /// Any duration type.
    AnyDuration,
    /// Any bit type.
    AnyBit,
    /// Any chars type.
    AnyChars,
    /// Any string type.
    AnyString,
    /// Any char type.
    AnyChar,
    /// Any date type.
    AnyDate,

    /// Type alias.
    Alias {
        /// Alias name.
        name: SmolStr,
        /// Target type.
        target: TypeId,
    },
}

impl Type {
    /// Returns true if this is a numeric type.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::SInt
                | Self::Int
                | Self::DInt
                | Self::LInt
                | Self::USInt
                | Self::UInt
                | Self::UDInt
                | Self::ULInt
                | Self::Real
                | Self::LReal
                | Self::Subrange { .. }
        )
    }

    /// Returns true if this is an integer type.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::SInt
                | Self::Int
                | Self::DInt
                | Self::LInt
                | Self::USInt
                | Self::UInt
                | Self::UDInt
                | Self::ULInt
                | Self::Subrange { .. }
        )
    }

    /// Returns true if this is a signed integer type.
    #[must_use]
    pub fn is_signed(&self) -> bool {
        matches!(self, Self::SInt | Self::Int | Self::DInt | Self::LInt)
    }

    /// Returns true if this is an unsigned integer type.
    #[must_use]
    pub fn is_unsigned(&self) -> bool {
        matches!(self, Self::USInt | Self::UInt | Self::UDInt | Self::ULInt)
    }

    /// Returns true if this is a floating point type.
    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Real | Self::LReal)
    }

    /// Returns true if this is a bit string type.
    #[must_use]
    pub fn is_bit_string(&self) -> bool {
        matches!(
            self,
            Self::Bool | Self::Byte | Self::Word | Self::DWord | Self::LWord
        )
    }

    /// Returns true if this is a string type.
    #[must_use]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String { .. } | Self::WString { .. })
    }

    /// Returns true if this is a character type.
    #[must_use]
    pub fn is_char(&self) -> bool {
        matches!(self, Self::Char | Self::WChar)
    }

    /// Returns true if this is a duration type.
    #[must_use]
    pub fn is_duration(&self) -> bool {
        matches!(self, Self::Time | Self::LTime)
    }

    /// Returns true if this is a date/time type.
    #[must_use]
    pub fn is_date(&self) -> bool {
        matches!(
            self,
            Self::Date | Self::LDate | Self::Tod | Self::LTod | Self::Dt | Self::Ldt
        )
    }

    /// Returns true if this is a time-related type (duration or date/time).
    #[must_use]
    pub fn is_time(&self) -> bool {
        matches!(
            self,
            Self::Time
                | Self::LTime
                | Self::Date
                | Self::LDate
                | Self::Tod
                | Self::LTod
                | Self::Dt
                | Self::Ldt
        )
    }

    /// Returns true if this is an elementary data type.
    #[must_use]
    pub fn is_elementary(&self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::SInt
                | Self::Int
                | Self::DInt
                | Self::LInt
                | Self::USInt
                | Self::UInt
                | Self::UDInt
                | Self::ULInt
                | Self::Real
                | Self::LReal
                | Self::Byte
                | Self::Word
                | Self::DWord
                | Self::LWord
                | Self::Time
                | Self::LTime
                | Self::Date
                | Self::LDate
                | Self::Tod
                | Self::LTod
                | Self::Dt
                | Self::Ldt
                | Self::String { .. }
                | Self::WString { .. }
                | Self::Char
                | Self::WChar
        )
    }

    /// Returns true if this is a derived data type.
    #[must_use]
    pub fn is_derived(&self) -> bool {
        matches!(
            self,
            Self::Array { .. }
                | Self::Struct { .. }
                | Self::Union { .. }
                | Self::Enum { .. }
                | Self::Pointer { .. }
                | Self::Reference { .. }
                | Self::FunctionBlock { .. }
                | Self::Class { .. }
                | Self::Interface { .. }
                | Self::Alias { .. }
        )
    }

    /// Returns true if this is a character or string type.
    #[must_use]
    pub fn is_chars(&self) -> bool {
        self.is_string() || self.is_char()
    }

    /// Returns the size of the type in bits, if known.
    #[must_use]
    pub fn bit_size(&self) -> Option<u32> {
        Some(match self {
            Self::Bool => 1,
            Self::SInt | Self::USInt | Self::Byte => 8,
            Self::Int | Self::UInt | Self::Word => 16,
            Self::DInt | Self::UDInt | Self::DWord | Self::Real | Self::Time => 32,
            Self::LInt | Self::ULInt | Self::LWord | Self::LReal | Self::LTime | Self::LDate => 64,
            Self::Char => 8,
            Self::WChar => 16,
            Self::Subrange { base, .. } => match *base {
                TypeId::SINT | TypeId::USINT | TypeId::BYTE => 8,
                TypeId::INT | TypeId::UINT | TypeId::WORD => 16,
                TypeId::DINT | TypeId::UDINT | TypeId::DWORD => 32,
                TypeId::LINT | TypeId::ULINT | TypeId::LWORD => 64,
                _ => return None,
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_helpers() {
        assert!(Type::Int.is_integer());
        assert!(Type::Bool.is_bit_string());
        assert!(Type::String { max_len: None }.is_string());
    }
}
