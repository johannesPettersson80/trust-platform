//! Type conversion functions.

#![allow(missing_docs)]

mod bcd;
mod bitstring;
mod dispatch;
mod numeric;
mod spec;
mod string;
mod time;
mod util;

use super::StandardLibrary;
use crate::error::RuntimeError;
use crate::value::Value;

pub(crate) use spec::ConversionSpec;

/// The rules of the type conversion functions (docs/specs/07-standard-functions.md 2.10).
/// `Iec` is truST's IEC 61131-3 behaviour; `Codesys` follows CODESYS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConversionProfile {
    #[default]
    Iec,
    Codesys,
}

/// The accepted `vendor_profile` names: the vendor profiles of `trust-lsp.toml`
/// (docs/public/reference/config/vendor-profiles.md) and `iec`.
pub const VENDOR_PROFILES: [&str; 7] = [
    "beckhoff",
    "codesys",
    "gxworks3",
    "iec",
    "mitsubishi",
    "siemens",
    "twincat",
];

impl ConversionProfile {
    /// The conversion rules of a `vendor_profile` name (trimmed, any case): `codesys`
    /// selects the CODESYS rules, `iec` and the vendor profiles without conversion rules of
    /// their own select the IEC rules; `None` for a name that is not in [`VENDOR_PROFILES`].
    #[must_use]
    pub fn from_vendor_profile(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "codesys" => Some(Self::Codesys),
            "iec" | "beckhoff" | "twincat" | "siemens" | "mitsubishi" | "gxworks3" => {
                Some(Self::Iec)
            }
            _ => None,
        }
    }

    /// The name of the rules: `iec` or `codesys`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Iec => "iec",
            Self::Codesys => "codesys",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ConversionMode {
    Round,
    Trunc,
}

pub fn register(_lib: &mut StandardLibrary) {}

pub fn is_conversion_name(name: &str) -> bool {
    conversion_spec(name).is_some()
}

pub fn call_conversion(name: &str, args: &[Value]) -> Option<Result<Value, RuntimeError>> {
    call_conversion_with(name, args, ConversionProfile::Iec)
}

/// A conversion function under the given rules.
pub fn call_conversion_with(
    name: &str,
    args: &[Value],
    profile: ConversionProfile,
) -> Option<Result<Value, RuntimeError>> {
    let spec = conversion_spec(name)?;
    Some(call_conversion_spec(spec, args, profile))
}

pub(crate) fn conversion_spec(name: &str) -> Option<ConversionSpec> {
    spec::parse_conversion_spec(name)
}

pub(crate) fn call_conversion_spec(
    spec: ConversionSpec,
    args: &[Value],
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    dispatch::apply_conversion(spec, args, profile)
}
