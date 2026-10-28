//! Type conversion functions.

mod bcd;
mod bitstring;
mod dispatch;
mod numeric;
mod spec;
mod string;
mod time;
mod types;
mod util;

pub use types::ConversionType;

use super::StandardLibrary;
use crate::error::RuntimeError;
use crate::value::Value;

pub use spec::ConversionSpec;

/// The rules of the type conversion functions (docs/specs/07-standard-functions.md 2.10).
/// `Iec` is truST's IEC 61131-3 behaviour; `Codesys` follows CODESYS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConversionProfile {
    /// truST's IEC 61131-3 conversion rules.
    #[default]
    Iec,
    /// The conversion rules of CODESYS, Beckhoff and TwinCAT projects.
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
    /// The conversion rules of a `vendor_profile` name (trimmed, any case): `codesys`,
    /// `beckhoff` and `twincat` (TwinCAT is built on CODESYS) select the CODESYS rules;
    /// `iec`, `siemens`, `mitsubishi` and `gxworks3` select the IEC rules; `None` for a name
    /// that is not in [`VENDOR_PROFILES`].
    #[must_use]
    pub fn from_vendor_profile(name: &str) -> Option<Self> {
        let name = name.trim();
        let named = |candidates: &[&str]| {
            candidates
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        };
        if named(&["codesys", "beckhoff", "twincat"]) {
            Some(Self::Codesys)
        } else if named(&["iec", "siemens", "mitsubishi", "gxworks3"]) {
            Some(Self::Iec)
        } else {
            None
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

/// Conversion recognition is algorithmic; no stored function registration is required.
pub fn register(_lib: &mut StandardLibrary) {}

/// Whether a name denotes a supported conversion syntax.
pub fn is_conversion_name(name: &str) -> bool {
    conversion_spec(name).is_some()
}

/// Recognize and execute a conversion, retaining recognition separately from errors.
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
    Some(call_conversion_spec_with(spec, args, profile))
}

/// Parse a conversion name to a reusable typed descriptor.
pub fn conversion_spec(name: &str) -> Option<ConversionSpec> {
    spec::parse_conversion_spec(name)
}

/// Execute a parsed conversion with its original width and fault rules, under the IEC rules.
pub fn call_conversion_spec(spec: ConversionSpec, args: &[Value]) -> Result<Value, RuntimeError> {
    call_conversion_spec_with(spec, args, ConversionProfile::Iec)
}

/// Execute a parsed conversion under the given rules.
pub fn call_conversion_spec_with(
    spec: ConversionSpec,
    args: &[Value],
    profile: ConversionProfile,
) -> Result<Value, RuntimeError> {
    dispatch::apply_conversion(spec, args, profile)
}
