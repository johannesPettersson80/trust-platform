//! The host's identify output (specification 11, "Identify"): what `trust-runtime` supplies to
//! the shared `identify` service (specification 34, SERVICE-07) and the load check that keeps the
//! program away from it.
//!
//! The output is one bit of the output process image or one GPIO line:
//! - an image bit is set in a copy of the output image that the cycle hands to the I/O drivers
//!   ([`ImageBit::set_in`]); the program's own image never holds it;
//! - a GPIO line is driven directly ([`GpioIdentifyLine`]).
//!
//! The state itself ([`HostIdentify`]) lives in an [`IdentifyHandle`], which the control state and
//! the runtime share.

use trust_runtime_services::identify::{Identify, IdentifyOutput};

use super::{GpioDriver, GpioIdentifyLine, IoAddress, IoSafeState, IoSize};
use crate::config::{IdentifyOutputConfig, IoConfig, IoDriverConfig};
use crate::error::RuntimeError;
use crate::memory::IoArea;
use crate::metrics::HostMonotonicClock;

mod handle;
pub use handle::IdentifyHandle;
pub(crate) use handle::IdleOutput;

/// One bit of the output process image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageBit {
    byte: usize,
    bit: u8,
}

impl ImageBit {
    /// The bit `address` names, or `None` for an address that is not one flat bit.
    fn of(address: &IoAddress) -> Option<Self> {
        let flat_bit = address.size == IoSize::Bit && !address.wildcard && address.path.len() == 1;
        flat_bit
            .then(|| usize::try_from(address.byte).ok())
            .flatten()
            .map(|byte| Self {
                byte,
                bit: address.bit,
            })
    }

    /// A copy of `image` with this bit set, or `None` when the bit lies beyond the image: the
    /// drivers always get an image of the length the program makes (specification 11,
    /// "Identify").
    #[must_use]
    pub fn set_in(self, image: &[u8]) -> Option<Vec<u8>> {
        let mut copy = image.to_vec();
        self.set_bit(&mut copy).then_some(copy)
    }

    /// Sets the bit in `image` itself; `false`, and nothing changed, when the image has no such
    /// byte.
    pub fn set_bit(self, image: &mut [u8]) -> bool {
        let Some(mask) = 1_u8.checked_shl(u32::from(self.bit)) else {
            return false;
        };
        match image.get_mut(self.byte) {
            Some(byte) => {
                *byte |= mask;
                true
            }
            None => false,
        }
    }
}

/// The output that `identify` drives on the host.
#[derive(Debug)]
pub enum HostIdentifyOutput {
    /// A bit of the output process image, and whether `identify` has it on.
    Image {
        /// The address the project wrote.
        address: IoAddress,
        /// The bit.
        bit: ImageBit,
        /// Whether the blink has the bit on.
        on: bool,
    },
    /// A GPIO line, driven directly.
    Gpio(GpioIdentifyLine),
}

impl HostIdentifyOutput {
    /// The image bit that the next publication of the outputs sets, while the blink has it on.
    #[must_use]
    pub fn image_bit_when_on(&self) -> Option<ImageBit> {
        match self {
            Self::Image { bit, on: true, .. } => Some(*bit),
            Self::Image { on: false, .. } | Self::Gpio(_) => None,
        }
    }

    /// The address of the output image bit, for an image output.
    #[must_use]
    pub fn image_address(&self) -> Option<&IoAddress> {
        match self {
            Self::Image { address, .. } => Some(address),
            Self::Gpio(_) => None,
        }
    }
}

impl IdentifyOutput for HostIdentifyOutput {
    fn prepare(&mut self) -> Result<(), RuntimeError> {
        match self {
            Self::Image { .. } => Ok(()),
            Self::Gpio(line) => line.prepare(),
        }
    }

    fn drive(&mut self, on: bool) -> Result<(), RuntimeError> {
        match self {
            Self::Image { on: level, .. } => {
                *level = on;
                Ok(())
            }
            Self::Gpio(line) => line.set(on),
        }
    }
}

/// The identify state of a hosted runtime: the host's monotonic clock and the output of
/// `[runtime.identify] output`.
pub type HostIdentify = Identify<HostMonotonicClock, HostIdentifyOutput>;

/// Builds the identify state for the output `setting` names, in the project whose `io.toml` is
/// `io`. A GPIO line is a line of the chip and backend of the first enabled `gpio` driver, or of
/// `/dev/gpiochip0` on the character-device backend without one. Nothing is requested from the
/// system until the first `identify`.
///
/// # Errors
///
/// The `gpio` driver's parameters are not the shape it accepts.
pub fn host_identify(
    setting: &IdentifyOutputConfig,
    io: &IoConfig,
) -> Result<HostIdentify, RuntimeError> {
    let output = match setting {
        IdentifyOutputConfig::Address(address) => HostIdentifyOutput::Image {
            address: address.clone(),
            bit: ImageBit::of(address).ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!("identify output {setting} is not one bit of the output image").into(),
                )
            })?,
            on: false,
        },
        IdentifyOutputConfig::GpioLine(line) => {
            let params = io.drivers.iter().find(|driver| is_enabled_gpio(driver));
            HostIdentifyOutput::Gpio(GpioIdentifyLine::new(
                params.map(|driver| &driver.params),
                *line,
            )?)
        }
    };
    Ok(Identify::new(HostMonotonicClock::default(), output))
}

/// Refuses a project whose program uses the identify output (specification 11, "Identify",
/// load), or whose output lies beyond the output image. `bindings` are the addresses of the
/// program's process image bindings, `output_image_len` the length of the output image they make,
/// and `io` the project's `io.toml`.
///
/// # Errors
///
/// [`RuntimeError::InvalidConfig`] naming the output and the binding or `io.toml` mapping that
/// uses it, or the image it lies beyond.
pub fn check_identify_output(
    setting: &IdentifyOutputConfig,
    bindings: &[IoAddress],
    output_image_len: usize,
    io: &IoConfig,
) -> Result<(), RuntimeError> {
    match setting {
        IdentifyOutputConfig::Address(address) => {
            check_address(setting, address, bindings, output_image_len)
        }
        IdentifyOutputConfig::GpioLine(line) => check_line(setting, *line, io),
    }
}

/// [`check_identify_output`] for the program that `runtime` runs: its `%Q` bindings and the length
/// of its output image. Every command that loads a project (`run`, `play`, `validate`, `check` and
/// `ide serve`) makes this check, so a project one of them accepts is a project `run` accepts.
///
/// # Errors
///
/// As [`check_identify_output`].
pub fn check_runtime_identify_output(
    setting: &IdentifyOutputConfig,
    runtime: &crate::CompiledRuntime,
    io: &IoConfig,
) -> Result<(), RuntimeError> {
    let bindings: Vec<IoAddress> = runtime
        .io_bindings()
        .iter()
        .map(|binding| binding.address.clone())
        .collect();
    check_identify_output(
        setting,
        &bindings,
        runtime.process_image_bytes(IoArea::Output),
        io,
    )
}

/// Refuses a program that replaces the running one (online change) when it uses the image bit
/// `address` that identify drives, or has no such bit in its output image (specification 11,
/// "Identify", load). `bindings` and `output_image_len` are the replacement's.
///
/// # Errors
///
/// [`RuntimeError::InvalidConfig`] as [`check_identify_output`] gives for an I/O address.
pub fn check_identify_replacement(
    address: &IoAddress,
    bindings: &[IoAddress],
    output_image_len: usize,
) -> Result<(), RuntimeError> {
    let setting = IdentifyOutputConfig::Address(address.clone());
    check_address(&setting, address, bindings, output_image_len)
}

fn check_address(
    setting: &IdentifyOutputConfig,
    address: &IoAddress,
    bindings: &[IoAddress],
    output_image_len: usize,
) -> Result<(), RuntimeError> {
    let inside_image = ImageBit::of(address).is_some_and(|bit| bit.byte < output_image_len);
    if !inside_image {
        let unit = if output_image_len == 1 {
            "byte"
        } else {
            "bytes"
        };
        return Err(refused(format!(
            "identify output {setting} is outside the output process image of \
             {output_image_len} {unit}"
        )));
    }
    match bindings.iter().find(|binding| covers(binding, address)) {
        Some(binding) => Err(refused(format!(
            "identify output {setting} is used by the program: it binds {binding}"
        ))),
        None => Ok(()),
    }
}

/// Whether an entry of `safe_state` gives the identify bit `address` a safe value: an entry for
/// the same bit, or for a byte, word, double word, long word or window of bytes that includes
/// its byte (specification 11, "Identify", "Ending").
#[must_use]
pub fn safe_state_holds_bit(safe_state: &IoSafeState, address: &IoAddress) -> bool {
    safe_outputs_hold_bit(&safe_state.outputs, address)
}

/// Whether one of the safe outputs `outputs` gives the identify bit `address` a safe value, as
/// [`safe_state_holds_bit`] says for a whole safe state.
pub fn safe_outputs_hold_bit(
    outputs: &[(IoAddress, crate::value::Value)],
    address: &IoAddress,
) -> bool {
    outputs.iter().any(|(entry, _)| covers(entry, address))
}

/// Whether the program's output `binding` covers the bit `address`: the same bit, or a byte,
/// word, double word, long word or window of bytes that includes its byte. A hierarchical
/// binding lives in its own address space and covers no flat bit.
fn covers(binding: &IoAddress, address: &IoAddress) -> bool {
    if binding.area != IoArea::Output || binding.wildcard || binding.path.len() != 1 {
        return false;
    }
    if binding.size == IoSize::Bit {
        return binding.byte == address.byte && binding.bit == address.bit;
    }
    binding
        .flat_byte_end()
        .ok()
        .flatten()
        .is_some_and(|end| binding.byte <= address.byte && address.byte < end)
}

fn check_line(
    setting: &IdentifyOutputConfig,
    line: u32,
    io: &IoConfig,
) -> Result<(), RuntimeError> {
    let wanted = line.to_string();
    for driver in io.drivers.iter().filter(|driver| is_enabled_gpio(driver)) {
        let (inputs, outputs) = GpioDriver::configuration_lines(&driver.params)?;
        let mapped = inputs
            .iter()
            .map(|entry| &entry.line)
            .chain(outputs.iter().map(|entry| &entry.line))
            .any(|mapped| mapped.as_str() == wanted);
        if mapped {
            return Err(refused(format!(
                "identify output {setting} is used by the project: io.toml maps GPIO line {line}"
            )));
        }
    }
    Ok(())
}

/// Whether `driver` is an enabled `gpio` driver of `io.toml`.
fn is_enabled_gpio(driver: &IoDriverConfig) -> bool {
    driver.enabled && driver.name.trim().eq_ignore_ascii_case("gpio")
}

fn refused(message: String) -> RuntimeError {
    RuntimeError::InvalidConfig(message.into())
}

#[cfg(test)]
mod tests;
