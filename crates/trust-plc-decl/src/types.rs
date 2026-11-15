//! The IEC types of a Rust POU's variables (specification 48 RU-25).

use std::borrow::Cow;

use syn::spanned::Spanned;
use syn::{Error, Expr, ExprLit, ExprUnary, GenericArgument, Lit, PathArguments, Type, UnOp};

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
    /// `STRING[n]`, written `Str<n>`: up to `n` bytes of UTF-8 (`Str` alone is `Str<80>`).
    Str(u16),
    /// `WSTRING[n]`, written `WStr<n>`: up to `n` UTF-16 code units.
    WStr(u16),
}

/// The capacity of a `STRING` or a `WSTRING` declared without one (specification 10 §2.3.1).
const DEFAULT_CAPACITY: u16 = 80;

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
    pub fn iec(self) -> Cow<'static, str> {
        match self {
            Self::Str(capacity) => Cow::Owned(format!("STRING[{capacity}]")),
            Self::WStr(capacity) => Cow::Owned(format!("WSTRING[{capacity}]")),
            other => Cow::Borrowed(other.name()),
        }
    }

    /// The IEC name of an elementary type without a capacity.
    fn name(self) -> &'static str {
        match self {
            Self::Str(_) => "STRING",
            Self::WStr(_) => "WSTRING",
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
            Self::Str(_) => "Str",
            Self::WStr(_) => "WStr",
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
            | Self::Ldt
            | Self::Str(_)
            | Self::WStr(_) => return None,
        };
        Some(range)
    }

    /// The type `ty` names. The attribute sees the type as written, so a variable's type is one of
    /// the table's names, not an alias of one.
    pub(crate) fn of(ty: &Type) -> Result<Self, Error> {
        if let Some(string) = string_type(ty) {
            return string;
        }
        let written = crate::last_path_segment(ty);
        written
            .and_then(|name| ALL.into_iter().find(|candidate| candidate.rust() == name))
            .ok_or_else(|| {
                let mut names: Vec<&str> = ALL.iter().map(|candidate| candidate.rust()).collect();
                names.extend(["Str<n>", "WStr<n>"]);
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

/// The string type `ty` names: `Str<n>` or `WStr<n>`, `Str` alone for `Str<80>`; `None` for another
/// type, and an error for a capacity that is not a whole number from 1 to 65,535 (specification 44
/// §7.6).
fn string_type(ty: &Type) -> Option<Result<Elementary, Error>> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let segment = path.path.segments.last()?;
    let wide = match segment.ident.to_string().as_str() {
        "Str" => false,
        "WStr" => true,
        _ => return None,
    };
    let capacity = match &segment.arguments {
        PathArguments::None => Some(DEFAULT_CAPACITY),
        PathArguments::AngleBracketed(arguments) if arguments.args.len() == 1 => {
            match arguments.args.first() {
                Some(GenericArgument::Const(Expr::Lit(ExprLit {
                    lit: Lit::Int(capacity),
                    ..
                }))) => capacity
                    .base10_parse::<u16>()
                    .ok()
                    .filter(|capacity| *capacity > 0),
                _ => None,
            }
        }
        _ => None,
    };
    Some(match capacity {
        Some(capacity) if wide => Ok(Elementary::WStr(capacity)),
        Some(capacity) => Ok(Elementary::Str(capacity)),
        None => Err(Error::new(
            segment.span(),
            "a string's capacity is a whole number from 1 to 65,535: `Str<20>`, `WStr<80>` \
             (specification 44 §7.6)",
        )),
    })
}

/// The IEC type of a Rust POU's variable (RU-25): an elementary type, or an array of a type of this
/// kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataType {
    /// An elementary type.
    Elementary(Elementary),
    /// `ARRAY[lower..lower + len - 1] OF element`, written `Array<element, lower, len>`.
    Array {
        /// The elements' type.
        element: Box<DataType>,
        /// The index of the first element.
        lower: i64,
        /// The number of elements, at least one.
        len: u32,
    },
}

impl DataType {
    /// The IEC type as the generated declaration writes it: `INT`, `STRING[20]`,
    /// `ARRAY[1..10] OF INT`.
    #[must_use]
    pub fn iec(&self) -> Cow<'static, str> {
        match self {
            Self::Elementary(elementary) => elementary.iec(),
            Self::Array {
                element,
                lower,
                len,
            } => Cow::Owned(format!(
                "ARRAY[{lower}..{}] OF {}",
                lower.saturating_add(i64::from(*len)).saturating_sub(1),
                element.iec()
            )),
        }
    }

    /// The elementary type; `None` for an array.
    #[must_use]
    pub fn elementary(&self) -> Option<Elementary> {
        match self {
            Self::Elementary(elementary) => Some(*elementary),
            Self::Array { .. } => None,
        }
    }

    /// The type `ty` names (RU-25).
    pub(crate) fn of(ty: &Type) -> Result<Self, Error> {
        match array_type(ty) {
            Some(array) => array,
            None => Elementary::of(ty).map(Self::Elementary),
        }
    }
}

/// The array type `ty` names, `Array<T, A, N>`: `N` elements of `T` indexed from `A`; `None` for
/// another type.
fn array_type(ty: &Type) -> Option<Result<DataType, Error>> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let segment = path.path.segments.last()?;
    if segment.ident != "Array" {
        return None;
    }
    let refused = || {
        Error::new(
            segment.span(),
            "an array is `Array<T, A, N>`: `N` elements of a type of specification 48 RU-25, \
             indexed from `A`; `Array<f64, 1, 10>` is `ARRAY[1..10] OF LREAL`",
        )
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Some(Err(refused()));
    };
    let arguments: Vec<&GenericArgument> = arguments.args.iter().collect();
    let [GenericArgument::Type(element), lower, len] = arguments.as_slice() else {
        return Some(Err(refused()));
    };
    let (Some(lower), Some(len)) = (constant(lower), constant(len)) else {
        return Some(Err(refused()));
    };
    let (Ok(lower), Ok(len)) = (i64::try_from(lower), u32::try_from(len)) else {
        return Some(Err(refused()));
    };
    if len == 0 || lower.checked_add(i64::from(len) - 1).is_none() {
        return Some(Err(refused()));
    }
    Some(DataType::of(element).map(|element| DataType::Array {
        element: Box::new(element),
        lower,
        len,
    }))
}

/// The value of the constant argument `argument`, an integer literal, `-` before one allowed.
fn constant(argument: &GenericArgument) -> Option<i128> {
    let GenericArgument::Const(expression) = argument else {
        return None;
    };
    let (negative, literal) = match expression {
        Expr::Lit(ExprLit {
            lit: Lit::Int(literal),
            ..
        }) => (false, literal),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => match &**expr {
            Expr::Lit(ExprLit {
                lit: Lit::Int(literal),
                ..
            }) => (true, literal),
            _ => return None,
        },
        _ => return None,
    };
    let value: i128 = literal.base10_parse().ok()?;
    Some(if negative { -value } else { value })
}
