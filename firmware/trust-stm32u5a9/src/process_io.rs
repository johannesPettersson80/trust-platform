//! The installed program's GPIO process image over the STM32U5A9's pins (specification 34, TARGET-01,
//! STATE-01, STATE-02 and STATE-04). Each line of the record is claimed by its pin name, and each
//! output is latched to its safe level before it becomes an output and then driven to its initial
//! level. The cycle reads and writes the image bits through [`GpioIo`].
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use trust_platform_stm32u5::gpio::{DigitalIo, Input, Output};
use trust_platform_stm32u5::pins::Pull;
use trust_runtime_core::deployment::{Configuration, GpioInput, GpioOutput};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::Value;
use trust_runtime_services::runner::engine::ProcessIo;
use trust_runtime_services::status::{IoDriverHealth, IoDriverStatus};

/// One line's place in the process image and its `io.toml` inversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bit {
    /// Byte of the image, from the line's `io.toml` address.
    byte: usize,
    /// Bit within the byte, 0 to 7.
    bit: u8,
    /// Whether the pin's level is inverted (`io.toml` `invert`).
    invert: bool,
}

impl Bit {
    /// The logical value of the physical level `high`; an inverted line flips it.
    fn value(self, high: bool) -> bool {
        high != self.invert
    }

    /// The physical level of the logical value `value`; an inverted line flips it.
    fn level(self, value: bool) -> bool {
        value != self.invert
    }

    fn of_address(address: &IoAddress, invert: bool) -> Self {
        Self {
            byte: address.byte as usize,
            bit: address.bit,
            invert,
        }
    }
}

/// The claimed GPIO lines of the installed application, in `io.toml` order.
pub struct GpioIo {
    io: DigitalIo,
    inputs: Vec<(Input, Bit)>,
    outputs: Vec<(Output, Bit)>,
}

impl GpioIo {
    /// Claims the record's `gpio` driver lines on `io`, in `io.toml` order. An output is latched to its
    /// safe level (the logical `[[io.safe_state]]` value, false without one) before it becomes an
    /// output, and then driven to its `initial` level, so the pin never shows a level other than its
    /// safe one before the program runs.
    ///
    /// # Errors
    ///
    /// The refusal's text: a driver other than `gpio` (the chip carries out only GPIO), or a line the
    /// chip cannot use, with the reason the adapter gives.
    pub fn claim(mut io: DigitalIo, config: &Configuration) -> Result<Self, String> {
        let (inputs, outputs) = lines(config)?;
        let mut claimed_inputs = Vec::with_capacity(inputs.len());
        for input in &inputs {
            let handle = io
                .claim_input(&input.line, Pull::Floating)
                .map_err(|error| format!("GPIO line {} cannot be claimed: {error}", input.line))?;
            claimed_inputs.push((handle, Bit::of_address(&input.address, input.invert)));
        }
        let mut claimed_outputs = Vec::with_capacity(outputs.len());
        for output in &outputs {
            let bit = Bit::of_address(&output.address, output.invert);
            let safe = safe_value(config, &output.address);
            let handle = io
                .claim_output(&output.line, bit.level(safe))
                .map_err(|error| format!("GPIO line {} cannot be claimed: {error}", output.line))?;
            io.drive(handle, bit.level(output.initial));
            claimed_outputs.push((handle, bit));
        }
        Ok(Self {
            io,
            inputs: claimed_inputs,
            outputs: claimed_outputs,
        })
    }
}

/// The `gpio` driver's lines of the record, in `io.toml` order. An enabled driver of another name is
/// refused with its name: the chip carries out only GPIO until its drivers exist (specification 34).
fn lines(config: &Configuration) -> Result<(Vec<GpioInput>, Vec<GpioOutput>), String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for driver in &config.io_drivers {
        if !driver.enabled {
            continue;
        }
        if driver.name.as_str() != "gpio" {
            return Err(format!(
                "the installed application uses the {} driver, which the STM32U5A9 does not carry out yet",
                driver.name
            ));
        }
        inputs.extend(driver.inputs.iter().cloned());
        outputs.extend(driver.outputs.iter().cloned());
    }
    Ok((inputs, outputs))
}

/// The logical safe value of the output at `address`: its `[[io.safe_state]]` entry when that is a
/// boolean, false otherwise (a de-energized output).
fn safe_value(config: &Configuration, address: &IoAddress) -> bool {
    config
        .safe_state
        .iter()
        .find(|entry| entry.address == *address)
        .is_some_and(|entry| matches!(entry.value, Value::Bool(true)))
}

impl ProcessIo for GpioIo {
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        for (input, bit) in &self.inputs {
            let high = self.io.read(*input);
            write_bit(image, *bit, bit.value(high))?;
        }
        Ok(())
    }

    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        for (output, bit) in &self.outputs {
            let value = read_bit(image, *bit)?;
            self.io.drive(*output, bit.level(value));
        }
        Ok(())
    }

    /// Every output is attempted, even when one image bit cannot be read, and the first failure is
    /// reported (specification 34, STATE-04).
    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let mut first = None;
        for (output, bit) in &self.outputs {
            match read_bit(image, *bit) {
                Ok(value) => self.io.drive(*output, bit.level(value)),
                Err(error) => {
                    if first.is_none() {
                        first = Some(error);
                    }
                }
            }
        }
        first.map_or(Ok(()), Err)
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        vec![IoDriverStatus {
            name: "gpio".into(),
            health: IoDriverHealth::Ok,
        }]
    }
}

/// Reads bit `bit` of the image.
fn read_bit(image: &[u8], bit: Bit) -> Result<bool, RuntimeError> {
    if bit.bit > 7 {
        return Err(invalid("gpio bit index must be between 0 and 7"));
    }
    let Some(value) = image.get(bit.byte) else {
        return Err(invalid("gpio output mapping outside the image"));
    };
    Ok(value & (1 << bit.bit) != 0)
}

/// Writes `value` to bit `bit` of the image.
fn write_bit(image: &mut [u8], bit: Bit, value: bool) -> Result<(), RuntimeError> {
    if bit.bit > 7 {
        return Err(invalid("gpio bit index must be between 0 and 7"));
    }
    let Some(target) = image.get_mut(bit.byte) else {
        return Err(invalid("gpio input mapping outside the image"));
    };
    if value {
        *target |= 1 << bit.bit;
    } else {
        *target &= !(1 << bit.bit);
    }
    Ok(())
}

/// A driver error with the message `message`.
fn invalid(message: &str) -> RuntimeError {
    RuntimeError::InvalidConfig(message.into())
}
