//! The `analog` driver's parameters and value rules (specification 11 §6.5): inputs that read the
//! voltage at a line in millivolts into a `%IW` word, and outputs that drive a line to the
//! millivolts of a `%QW` word.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;

use super::{parse_root, AddressKind, Once, ParamsError, Ranges, Table, LINE, LINE_ONCE};

/// The `analog` driver's parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalogParams {
    /// `inputs`, in `io.toml` order.
    pub inputs: Vec<AnalogInput>,
    /// `outputs`, in `io.toml` order.
    pub outputs: Vec<AnalogOutput>,
}

/// One analog input: the millivolts at `line` into the `%IW` word `address`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalogInput {
    /// `address`, a concrete `%IW` word.
    pub address: IoAddress,
    /// `line`: a chip's pin such as `GPIO2`, or a Linux IIO channel such as
    /// `iio:device0/in_voltage1`.
    pub line: SmolStr,
}

/// One analog output: the line driven to the millivolts of the `%QW` word `address`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalogOutput {
    /// `address`, a concrete `%QW` word.
    pub address: IoAddress,
    /// `line`: a chip's converter output, or a Linux IIO channel such as `iio:device0/out_voltage0`.
    pub line: SmolStr,
    /// `initial`: the millivolts before the program writes the output; default 0.
    pub initial: i16,
}

/// An input's address: a `%IW` word.
const INPUT_ADDRESS: AddressKind = AddressKind {
    area: IoArea::Input,
    size: IoSize::Word,
    prefix: "%IW",
    noun: "word",
};

/// An output's address: a `%QW` word.
const OUTPUT_ADDRESS: AddressKind = AddressKind {
    area: IoArea::Output,
    size: IoSize::Word,
    prefix: "%QW",
    noun: "word",
};

impl AnalogParams {
    /// The parameters of the JSON text `text`, the driver's `params` table (specification 34
    /// §5.1.1, section 8), checked by the rules of specification 11 §6.5.
    ///
    /// # Errors
    ///
    /// The first key that breaks a rule, with the reason.
    pub fn from_json(text: &str) -> Result<Self, ParamsError> {
        let map = parse_root(text)?;
        let root = Table::root(&map);
        root.only(&["inputs", "outputs"], "the analog driver")?;
        let mut lines = Once::new(LINE_ONCE);
        let mut input_bytes = Ranges::new();
        let mut inputs = Vec::new();
        for entry in root.list("inputs")? {
            entry.only(&["address", "line"], "an analog input")?;
            let address = entry.address(&INPUT_ADDRESS)?;
            input_bytes.claim(&entry, &address, 2, INPUT_ADDRESS.prefix)?;
            let line = entry.required_line("line", LINE, &mut lines)?;
            inputs.push(AnalogInput { address, line });
        }
        let mut output_bytes = Ranges::new();
        let mut outputs = Vec::new();
        for entry in root.list("outputs")? {
            entry.only(&["address", "initial", "line"], "an analog output")?;
            let address = entry.address(&OUTPUT_ADDRESS)?;
            output_bytes.claim(&entry, &address, 2, OUTPUT_ADDRESS.prefix)?;
            let line = entry.required_line("line", LINE, &mut lines)?;
            let initial = entry.integer("initial", i16::MIN, i16::MAX)?.unwrap_or(0);
            outputs.push(AnalogOutput {
                address,
                line,
                initial,
            });
        }
        Ok(Self { inputs, outputs })
    }
}

/// The `INT` an input reads for `millivolts`, limited to −32,768 and 32,767 (specification 11
/// §6.5: a value outside what the hardware converts is limited to its range).
#[must_use]
pub fn input_value(millivolts: i32) -> i16 {
    i16::try_from(millivolts).unwrap_or(if millivolts < 0 { i16::MIN } else { i16::MAX })
}

/// The millivolts of a Linux IIO input: `(raw + offset) × scale`, rounded to the nearest millivolt,
/// where `scale` is the channel's millivolts per count (specification 11 §6.5). A half rounds away
/// from zero, a result beyond an `i32` is limited to it, and a result that is not a number, from a
/// `scale` or an `offset` that is not one, is 0.
#[must_use]
#[allow(clippy::cast_precision_loss)] // an IIO count is far below 2^53, where an f64 is exact
pub fn iio_millivolts(raw: i64, offset: f64, scale: f64) -> i32 {
    round_to_i32((raw as f64 + offset) * scale)
}

/// `value` rounded to the nearest integer, a half away from zero, and limited to an `i32`; a value
/// that is not a number is 0. Rounded by hand: a chip has no `f64::round` without the standard
/// library.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // see the two casts
fn round_to_i32(value: f64) -> i32 {
    // `as` rounds toward zero, limits to an i64 and makes NaN 0.
    let whole = value as i64;
    // Exact: below 2^52 a value less its whole part loses no bit, and from 2^52 on every f64 is
    // whole; beyond an i64 the limits below hold whatever the fraction is.
    let fraction = value - whole as f64;
    let rounded = if fraction >= 0.5 {
        whole.saturating_add(1)
    } else if fraction <= -0.5 {
        whole.saturating_sub(1)
    } else {
        whole
    };
    i32::try_from(rounded).unwrap_or(if rounded < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests;
