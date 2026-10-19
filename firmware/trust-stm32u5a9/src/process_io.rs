//! The installed program's GPIO process image over the STM32U5A9's pins (specification 34, TARGET-01,
//! STATE-01, STATE-02 and STATE-04). The chip claims each line of the record by its pin name, and each
//! output is latched to its safe level before it becomes an output and then driven to its initial level.
//! The image reads and drives the bits through the shared process image of
//! `trust_runtime_services::chip::process_io`; this file keeps the chip's claim.
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use trust_platform_stm32u5::gpio::{DigitalIo, Input, Output};
use trust_platform_stm32u5::pins::Pull;
use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_services::chip::process_io::{
    record_lines, safe_value, GpioPins, ImageBit, ProcessImage,
};

/// The STM32U5A9's claimed pins behind the shared process image: an input is read, and an output is
/// driven, through the handle its claim returned (specification 34, TARGET-01).
pub struct Pins(DigitalIo);

impl GpioPins for Pins {
    type Input = Input;
    type Output = Output;

    fn is_high(&self, input: &Input) -> bool {
        self.0.read(*input)
    }

    fn drive(&mut self, output: &mut Output, high: bool) {
        self.0.drive(*output, high);
    }
}

/// The installed program's GPIO process image over the STM32U5A9's pins, one entry per configured line in
/// `io.toml` order.
pub type GpioIo = ProcessImage<Pins>;

/// Claims the record's `gpio` driver lines on `io`, in `io.toml` order. An output is latched to its safe
/// level (the logical `[[io.safe_state]]` value, false without one) before it becomes an output, and then
/// driven to its `initial` level, so the pin never shows a level other than its safe one before the
/// program runs.
///
/// # Errors
///
/// The refusal's text: a driver other than `gpio` (the chip carries out only GPIO), or a line the chip
/// cannot use, with the reason the adapter gives.
pub fn claim(mut io: DigitalIo, config: &Configuration) -> Result<GpioIo, String> {
    let (inputs, outputs) = record_lines(config, "STM32U5A9")?;
    let mut claimed_inputs = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let handle = io
            .claim_input(&input.line, Pull::Floating)
            .map_err(|error| format!("GPIO line {} cannot be claimed: {error}", input.line))?;
        claimed_inputs.push((handle, image_bit(&input.address, input.invert)));
    }
    let mut claimed_outputs = Vec::with_capacity(outputs.len());
    for output in &outputs {
        let bit = image_bit(&output.address, output.invert);
        let safe = safe_value(config, &output.address);
        let handle = io
            .claim_output(&output.line, bit.level(safe))
            .map_err(|error| format!("GPIO line {} cannot be claimed: {error}", output.line))?;
        io.drive(handle, bit.level(output.initial));
        claimed_outputs.push((handle, bit));
    }
    Ok(ProcessImage::new(Pins(io), claimed_inputs, claimed_outputs))
}

/// The line's place in the flat process image: the byte and bit its `io.toml` address names, and its
/// inversion.
fn image_bit(address: &IoAddress, invert: bool) -> ImageBit {
    ImageBit {
        byte: address.byte as usize,
        bit: address.bit,
        invert,
    }
}
