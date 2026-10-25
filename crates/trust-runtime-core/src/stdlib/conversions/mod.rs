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
    let spec = conversion_spec(name)?;
    Some(call_conversion_spec(spec, args))
}

/// Parse a conversion name to a reusable typed descriptor.
pub fn conversion_spec(name: &str) -> Option<ConversionSpec> {
    spec::parse_conversion_spec(name)
}

/// Execute a parsed conversion with its original width and fault rules.
pub fn call_conversion_spec(spec: ConversionSpec, args: &[Value]) -> Result<Value, RuntimeError> {
    dispatch::apply_conversion(spec, args)
}
