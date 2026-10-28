//! `[runtime.identify] output`: the one output that `identify` blinks (specification 11,
//! "Identify").

use std::fmt;

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSize};
use crate::memory::IoArea;

/// The prefix of a GPIO line, `gpio:<line>`.
const GPIO_PREFIX: &str = "gpio:";

/// The output that `identify` blinks, as `[runtime.identify] output` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifyOutputConfig {
    /// One bit of the output process image, `%QX<byte>.<bit>`: the runtime sets the bit in the
    /// copy of the output image that it hands to the I/O drivers.
    Address(IoAddress),
    /// A GPIO line, `gpio:<line>`: the runtime drives the line directly.
    GpioLine(u32),
}

impl IdentifyOutputConfig {
    /// Reads the value of `[runtime.identify] output`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidConfig`] when the text is neither a single `%Q` bit address
    /// (`%QX0.7`: no wildcard, no input or marker address, no byte, word or larger address, no
    /// hierarchical address) nor `gpio:` and a line number in decimal.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(refused("must not be empty"));
        }
        if text.starts_with('%') {
            return parse_address(text);
        }
        if let Some(prefix) = text.get(..GPIO_PREFIX.len())
            && prefix.eq_ignore_ascii_case(GPIO_PREFIX)
        {
            return parse_line(&text[GPIO_PREFIX.len()..], text);
        }
        Err(refused(&format!(
            "'{text}' must be an I/O address such as %QX0.7 or a GPIO line such as gpio:17"
        )))
    }
}

/// An I/O address of the form `%QX<byte>.<bit>`.
fn parse_address(text: &str) -> Result<IdentifyOutputConfig, RuntimeError> {
    let address = IoAddress::parse(text)
        .map_err(|_| refused(&format!("'{text}' is not an I/O address such as %QX0.7")))?;
    // The canonical text also refuses a shorthand such as `%QX5` or leading zeros, so the setting
    // always says which byte and which bit it means.
    let single_output_bit = address.area == IoArea::Output
        && address.size == IoSize::Bit
        && !address.wildcard
        && address.path.len() == 1
        && address.to_string() == text;
    if single_output_bit {
        Ok(IdentifyOutputConfig::Address(address))
    } else {
        Err(refused(&format!(
            "'{text}' must be one bit of the output process image, written %QX<byte>.<bit>"
        )))
    }
}

/// The line number of `gpio:<line>`: decimal digits only, as the `line` of a `gpio` driver is on
/// a computer.
fn parse_line(digits: &str, text: &str) -> Result<IdentifyOutputConfig, RuntimeError> {
    let line = if digits.bytes().all(|byte| byte.is_ascii_digit()) {
        digits.parse::<u32>().ok()
    } else {
        None
    };
    line.map(IdentifyOutputConfig::GpioLine).ok_or_else(|| {
        refused(&format!(
            "'{text}' must name a GPIO line by its number, for example gpio:17"
        ))
    })
}

fn refused(reason: &str) -> RuntimeError {
    RuntimeError::InvalidConfig(SmolStr::new(format!("runtime.identify.output {reason}")))
}

impl fmt::Display for IdentifyOutputConfig {
    /// The output as `runtime.toml` writes it, for the errors that name it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(address) => write!(formatter, "{address}"),
            Self::GpioLine(line) => write!(formatter, "{GPIO_PREFIX}{line}"),
        }
    }
}
