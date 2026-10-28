//! The `pwm` driver's parameters and duty rules (specification 11 §6.5): outputs that drive a line
//! at a fixed frequency with the duty cycle of a `%QW` word, in hundredths of a percent.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;

use super::{parse_root, AddressKind, Once, ParamsError, Ranges, Table, LINE, LINE_ONCE};

/// The largest duty cycle, 100.00 %, in hundredths of a percent; a larger value drives 100 %.
pub const FULL_DUTY: u16 = 10_000;

/// The highest `frequency_hz`.
const MAX_FREQUENCY_HZ: u32 = 1_000_000;

/// An output's address: a `%QW` word.
const ADDRESS: AddressKind = AddressKind {
    area: IoArea::Output,
    size: IoSize::Word,
    prefix: "%QW",
    noun: "word",
};

/// The `pwm` driver's parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PwmParams {
    /// `outputs`, in `io.toml` order.
    pub outputs: Vec<PwmOutput>,
}

/// One PWM output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PwmOutput {
    /// `address`, a concrete `%QW` word: the duty cycle in hundredths of a percent.
    pub address: IoAddress,
    /// `line`: a chip's pin such as `GPIO4`, or a Linux sysfs PWM channel such as `pwmchip0/pwm1`.
    pub line: SmolStr,
    /// `frequency_hz`, from 1 to 1,000,000; required.
    pub frequency_hz: u32,
    /// `initial`: the duty cycle before the program writes the output; default 0.
    pub initial: u16,
}

impl PwmParams {
    /// The parameters of the JSON text `text`, the driver's `params` table (specification 34
    /// §5.1.1, section 8), checked by the rules of specification 11 §6.5.
    ///
    /// # Errors
    ///
    /// The first key that breaks a rule, with the reason.
    pub fn from_json(text: &str) -> Result<Self, ParamsError> {
        let map = parse_root(text)?;
        let root = Table::root(&map);
        root.only(&["outputs"], "the pwm driver")?;
        let mut lines = Once::new(LINE_ONCE);
        let mut bytes = Ranges::new();
        let mut outputs = Vec::new();
        for entry in root.list("outputs")? {
            entry.only(
                &["address", "frequency_hz", "initial", "line"],
                "a pwm output",
            )?;
            let address = entry.address(&ADDRESS)?;
            bytes.claim(&entry, &address, 2, ADDRESS.prefix)?;
            let line = entry.required_line("line", LINE, &mut lines)?;
            let frequency_hz = entry.required_integer("frequency_hz", 1, MAX_FREQUENCY_HZ)?;
            let initial = entry.integer("initial", 0, FULL_DUTY)?.unwrap_or(0);
            outputs.push(PwmOutput {
                address,
                line,
                frequency_hz,
                initial,
            });
        }
        Ok(Self { outputs })
    }
}

/// The finest duty resolution, in bits, that a timer clocked at `clock_hz` has at `frequency_hz`,
/// at most `max_bits`: the largest `bits` with `frequency_hz × 2^bits ≤ clock_hz`. `None` when not
/// even one bit fits, a frequency the hardware cannot make, and for a frequency of 0.
#[must_use]
pub fn resolution_bits(clock_hz: u32, frequency_hz: u32, max_bits: u8) -> Option<u8> {
    // `2^bits ≤ clock_hz / frequency_hz` holds for the quotient rounded down as well, as `2^bits`
    // is whole: the largest such `bits` is that quotient's base-2 logarithm, rounded down.
    let bits = clock_hz
        .checked_div(frequency_hz)?
        .checked_ilog2()?
        .min(u32::from(max_bits));
    u8::try_from(bits).ok().filter(|bits| *bits >= 1)
}

/// The compare value of the duty cycle `duty` (hundredths of a percent; above [`FULL_DUTY`] it is
/// 100 %) at `bits` of resolution: `duty × 2^bits / 10,000`, rounded to the nearest count, a half
/// up. A value above `u32::MAX`, from 32 bits on, is `u32::MAX`.
#[must_use]
pub fn duty_ticks(duty: u16, bits: u8) -> u32 {
    let duty = u128::from(duty.min(FULL_DUTY));
    if duty == 0 {
        return 0;
    }
    let full = u128::from(FULL_DUTY);
    let ticks = 1u128
        .checked_shl(u32::from(bits))
        .and_then(|counts| counts.checked_mul(duty))
        .map_or(u128::MAX, |scaled| scaled.saturating_add(full / 2) / full);
    u32::try_from(ticks).unwrap_or(u32::MAX)
}

/// The `duty_cycle` in nanoseconds that a Linux sysfs PWM channel takes for `duty` (hundredths of a
/// percent) of `period_ns`: `period_ns × duty / 10,000`, rounded down; above [`FULL_DUTY`] it is
/// `period_ns`.
#[must_use]
pub fn duty_nanos(duty: u16, period_ns: u64) -> u64 {
    let duty = u128::from(duty.min(FULL_DUTY));
    let nanos = u128::from(period_ns) * duty / u128::from(FULL_DUTY);
    // At most `period_ns`, so it always fits.
    u64::try_from(nanos).unwrap_or(period_ns)
}

#[cfg(test)]
mod tests;
