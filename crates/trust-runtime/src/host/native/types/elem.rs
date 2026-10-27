//! The elementary types (IEC §6.4.2, Table 10) as the generated Rust and the type table hold them
//! (specification 44 §5.8, §6.2).

use trust_hir::Type;
use trust_runtime_core::native::image::type_kind;

/// An elementary type (IEC §6.4.2, Table 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::native) enum Elem {
    Bool,
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Real,
    LReal,
    Byte,
    Word,
    DWord,
    LWord,
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    Dt,
    Ldt,
    Char,
    WChar,
}

impl Elem {
    /// The elementary type of a registry type, when it is one.
    pub(super) fn of(ty: &Type) -> Option<Self> {
        Some(match ty {
            Type::Bool => Self::Bool,
            Type::SInt => Self::SInt,
            Type::Int => Self::Int,
            Type::DInt => Self::DInt,
            Type::LInt => Self::LInt,
            Type::USInt => Self::USInt,
            Type::UInt => Self::UInt,
            Type::UDInt => Self::UDInt,
            Type::ULInt => Self::ULInt,
            Type::Real => Self::Real,
            Type::LReal => Self::LReal,
            Type::Byte => Self::Byte,
            Type::Word => Self::Word,
            Type::DWord => Self::DWord,
            Type::LWord => Self::LWord,
            Type::Time => Self::Time,
            Type::LTime => Self::LTime,
            Type::Date => Self::Date,
            Type::LDate => Self::LDate,
            Type::Tod => Self::Tod,
            Type::LTod => Self::LTod,
            Type::Dt => Self::Dt,
            Type::Ldt => Self::Ldt,
            Type::Char => Self::Char,
            Type::WChar => Self::WChar,
            _ => return None,
        })
    }

    /// The bytes of a value.
    pub(in crate::native) fn size(self) -> u32 {
        match self {
            Self::Bool | Self::SInt | Self::USInt | Self::Byte | Self::Char => 1,
            Self::Int | Self::UInt | Self::Word | Self::WChar => 2,
            Self::DInt | Self::UDInt | Self::DWord | Self::Real => 4,
            _ => 8,
        }
    }

    /// The Rust type of a value: `bool` for `BOOL`, the integer or float of the declared size,
    /// `i64` nanoseconds for every temporal type (specification 10's storage).
    pub(in crate::native) fn rust(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::SInt => "i8",
            Self::Int => "i16",
            Self::DInt => "i32",
            Self::LInt
            | Self::Time
            | Self::LTime
            | Self::Date
            | Self::LDate
            | Self::Tod
            | Self::LTod
            | Self::Dt
            | Self::Ldt => "i64",
            Self::USInt | Self::Byte | Self::Char => "u8",
            Self::UInt | Self::Word | Self::WChar => "u16",
            Self::UDInt | Self::DWord => "u32",
            Self::ULInt | Self::LWord => "u64",
            Self::Real => "f32",
            Self::LReal => "f64",
        }
    }

    /// The Rust type a value is stored as: a `BOOL` as a byte, 0 or 1 (specification 44 §6.2).
    pub(in crate::native) fn stored(self) -> &'static str {
        match self {
            Self::Bool => "u8",
            other => other.rust(),
        }
    }

    /// The type table's kind (specification 44 §5.8).
    pub(in crate::native) fn kind(self) -> u32 {
        match self {
            Self::Bool => type_kind::BOOL,
            Self::SInt => type_kind::SINT,
            Self::Int => type_kind::INT,
            Self::DInt => type_kind::DINT,
            Self::LInt => type_kind::LINT,
            Self::USInt => type_kind::USINT,
            Self::UInt => type_kind::UINT,
            Self::UDInt => type_kind::UDINT,
            Self::ULInt => type_kind::ULINT,
            Self::Real => type_kind::REAL,
            Self::LReal => type_kind::LREAL,
            Self::Byte => type_kind::BYTE,
            Self::Word => type_kind::WORD,
            Self::DWord => type_kind::DWORD,
            Self::LWord => type_kind::LWORD,
            Self::Time => type_kind::TIME,
            Self::LTime => type_kind::LTIME,
            Self::Date => type_kind::DATE,
            Self::LDate => type_kind::LDATE,
            Self::Tod => type_kind::TOD,
            Self::LTod => type_kind::LTOD,
            Self::Dt => type_kind::DT,
            Self::Ldt => type_kind::LDT,
            Self::Char => type_kind::CHAR,
            Self::WChar => type_kind::WCHAR,
        }
    }

    /// The IEC name.
    pub(in crate::native) fn name(self) -> &'static str {
        match self {
            Self::Bool => "BOOL",
            Self::SInt => "SINT",
            Self::Int => "INT",
            Self::DInt => "DINT",
            Self::LInt => "LINT",
            Self::USInt => "USINT",
            Self::UInt => "UINT",
            Self::UDInt => "UDINT",
            Self::ULInt => "ULINT",
            Self::Real => "REAL",
            Self::LReal => "LREAL",
            Self::Byte => "BYTE",
            Self::Word => "WORD",
            Self::DWord => "DWORD",
            Self::LWord => "LWORD",
            Self::Time => "TIME",
            Self::LTime => "LTIME",
            Self::Date => "DATE",
            Self::LDate => "LDATE",
            Self::Tod => "TIME_OF_DAY",
            Self::LTod => "LTIME_OF_DAY",
            Self::Dt => "DATE_AND_TIME",
            Self::Ldt => "LDATE_AND_TIME",
            Self::Char => "CHAR",
            Self::WChar => "WCHAR",
        }
    }

    pub(in crate::native) fn is_signed(self) -> bool {
        matches!(self, Self::SInt | Self::Int | Self::DInt | Self::LInt)
    }

    pub(in crate::native) fn is_unsigned(self) -> bool {
        matches!(self, Self::USInt | Self::UInt | Self::UDInt | Self::ULInt)
    }

    pub(in crate::native) fn is_bits(self) -> bool {
        matches!(self, Self::Byte | Self::Word | Self::DWord | Self::LWord)
    }

    pub(in crate::native) fn is_integer(self) -> bool {
        self.is_signed() || self.is_unsigned()
    }

    pub(in crate::native) fn is_real(self) -> bool {
        matches!(self, Self::Real | Self::LReal)
    }

    pub(in crate::native) fn is_duration(self) -> bool {
        matches!(self, Self::Time | Self::LTime)
    }
}
