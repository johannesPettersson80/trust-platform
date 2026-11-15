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
}

const ALL: [Elementary; 11] = [
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
            Self::Bool | Self::Real | Self::Lreal => return None,
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
