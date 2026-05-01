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
    let spec = conversion_spec(name)?;
    Some(call_conversion_spec(spec, args))
}

pub(crate) fn conversion_spec(name: &str) -> Option<ConversionSpec> {
    spec::parse_conversion_spec(name)
}

pub(crate) fn call_conversion_spec(
    spec: ConversionSpec,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    dispatch::apply_conversion(spec, args)
}
