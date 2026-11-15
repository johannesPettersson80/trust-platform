//! The IEC types of a Rust POU's variables (specification 48 RU-25).

use syn::spanned::Spanned;
use syn::{Error, Type};

/// An elementary IEC type, which a Rust POU writes as the Rust type of RU-25's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elementary {
    /// `BOOL`, written `bool`.
    Bool,
    /// `SINT`, written `i8`.
    Sint,
    /// `INT`, written `i16`.
    Int,
    /// `DINT`, written `i32`.
    Dint,
    /// `LINT`, written `i64`.
    Lint,
    /// `USINT`, written `u8`.
    Usint,
    /// `UINT`, written `u16`.
    Uint,
    /// `UDINT`, written `u32`.
    Udint,
    /// `ULINT`, written `u64`.
    Ulint,
    /// `REAL`, written `f32`.
    Real,
    /// `LREAL`, written `f64`.
    Lreal,
    /// `BYTE`, written `Byte`.
    Byte,
    /// `WORD`, written `Word`.
    Word,
    /// `DWORD`, written `Dword`.
    Dword,
    /// `LWORD`, written `Lword`.
    Lword,
    /// `CHAR`, written `Char`.
    Char,
    /// `WCHAR`, written `WChar`.
    WChar,
    /// `TIME`, written `Time`.
    Time,
    /// `LTIME`, written `LTime`.
    LTime,
    /// `DATE`, written `Date`.
    Date,
    /// `LDATE`, written `LDate`.
    LDate,
    /// `TIME_OF_DAY`, written `Tod`.
    Tod,
    /// `LTIME_OF_DAY`, written `LTod`.
    LTod,
    /// `DATE_AND_TIME`, written `Dt`.
    Dt,
    /// `LDATE_AND_TIME`, written `Ldt`.
    Ldt,
}

const ALL: [Elementary; 25] = [
    Elementary::Bool,
    Elementary::Sint,
    Elementary::Int,
    Elementary::Dint,
    Elementary::Lint,
    Elementary::Usint,
    Elementary::Uint,
    Elementary::Udint,
    Elementary::Ulint,
    Elementary::Real,
    Elementary::Lreal,
    Elementary::Byte,
    Elementary::Word,
    Elementary::Dword,
    Elementary::Lword,
    Elementary::Char,
    Elementary::WChar,
    Elementary::Time,
    Elementary::LTime,
    Elementary::Date,
    Elementary::LDate,
    Elementary::Tod,
    Elementary::LTod,
    Elementary::Dt,
    Elementary::Ldt,
];

impl Elementary {
    /// The IEC name, as the generated declaration writes it.
    #[must_use]
    pub fn iec(self) -> &'static str {
        match self {
            Self::Bool => "BOOL",
            Self::Sint => "SINT",
            Self::Int => "INT",
            Self::Dint => "DINT",
            Self::Lint => "LINT",
            Self::Usint => "USINT",
            Self::Uint => "UINT",
            Self::Udint => "UDINT",
            Self::Ulint => "ULINT",
            Self::Real => "REAL",
            Self::Lreal => "LREAL",
            Self::Byte => "BYTE",
            Self::Word => "WORD",
            Self::Dword => "DWORD",
            Self::Lword => "LWORD",
            Self::Char => "CHAR",
            Self::WChar => "WCHAR",
            Self::Time => "TIME",
            Self::LTime => "LTIME",
            Self::Date => "DATE",
            Self::LDate => "LDATE",
            Self::Tod => "TOD",
            Self::LTod => "LTOD",
            Self::Dt => "DT",
            Self::Ldt => "LDT",
        }
    }

    /// The Rust type a Rust POU writes.
    #[must_use]
    pub fn rust(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Sint => "i8",
            Self::Int => "i16",
            Self::Dint => "i32",
            Self::Lint => "i64",
            Self::Usint => "u8",
            Self::Uint => "u16",
            Self::Udint => "u32",
            Self::Ulint => "u64",
            Self::Real => "f32",
            Self::Lreal => "f64",
            Self::Byte => "Byte",
            Self::Word => "Word",
            Self::Dword => "Dword",
            Self::Lword => "Lword",
            Self::Char => "Char",
            Self::WChar => "WChar",
            Self::Time => "Time",
            Self::LTime => "LTime",
            Self::Date => "Date",
            Self::LDate => "LDate",
            Self::Tod => "Tod",
            Self::LTod => "LTod",
            Self::Dt => "Dt",
            Self::Ldt => "Ldt",
        }
    }

    /// The bits of a bit string (IEC §6.4.2, Table 10); `None` for another type.
    #[must_use]
    pub fn bits(self) -> Option<u32> {
        match self {
            Self::Byte => Some(8),
            Self::Word => Some(16),
            Self::Dword => Some(32),
            Self::Lword => Some(64),
            _ => None,
        }
    }

    /// The smallest and the largest value of an integer type (IEC §6.4.2, Table 10); `None` for
    /// `BOOL` and the reals.
    #[must_use]
    pub fn range(self) -> Option<(i128, i128)> {
        let range = match self {
            Self::Sint => (i8::MIN.into(), i8::MAX.into()),
            Self::Int => (i16::MIN.into(), i16::MAX.into()),
            Self::Dint => (i32::MIN.into(), i32::MAX.into()),
            Self::Lint => (i64::MIN.into(), i64::MAX.into()),
            Self::Usint => (0, u8::MAX.into()),
            Self::Uint => (0, u16::MAX.into()),
            Self::Udint => (0, u32::MAX.into()),
            Self::Ulint => (0, u64::MAX.into()),
            Self::Bool
            | Self::Real
            | Self::Lreal
            | Self::Byte
            | Self::Word
            | Self::Dword
            | Self::Lword
            | Self::Char
            | Self::WChar
            | Self::Time
            | Self::LTime
            | Self::Date
            | Self::LDate
            | Self::Tod
            | Self::LTod
            | Self::Dt
            | Self::Ldt => return None,
        };
        Some(range)
    }

    /// The type `ty` names. The attribute sees the type as written, so a variable's type is one of
    /// the table's names, not an alias of one.
    pub(crate) fn of(ty: &Type) -> Result<Self, Error> {
        let written = crate::last_path_segment(ty);
        written
            .and_then(|name| ALL.into_iter().find(|candidate| candidate.rust() == name))
            .ok_or_else(|| {
                let names: Vec<&str> = ALL.iter().map(|candidate| candidate.rust()).collect();
                Error::new(
                    ty.span(),
                    format!(
                        "a Rust POU's variable has one of the types of specification 48 RU-25, \
                         which this release gives as {}",
                        names.join(", ")
                    ),
                )
            })
    }
}
