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

/// The rules of the type conversion functions (docs/specs/07-standard-functions.md).
/// `Iec` is truST's IEC 61131-3 behaviour; `Codesys` follows CODESYS (section 2.7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConversionProfile {
    #[default]
    Iec,
    Codesys,
}

impl ConversionProfile {
    /// The profile of a `vendor_profile` name (`codesys`, `iec`); `None` when the vendor
    /// has no conversion profile.
    #[must_use]
    pub fn from_vendor_profile(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "iec" => Some(Self::Iec),
            "codesys" => Some(Self::Codesys),
            _ => None,
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
