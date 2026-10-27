//! Terminal fault boundary: drive every configured output to its safe level, write the cause and
//! location into the persistent reset record, and reset the chip (design §1.1 and R4).
//!
//! The configured outputs are the GPIO lines' outputs and the `pwm` driver's outputs, whose pins
//! the GPIO matrix routes to an LEDC channel: the fault path routes each of them to its GPIO latch
//! before it drives the latch, so a PWM output stops at its safe level as a GPIO output does.
//!
//! The control link carries replies only, so a fault never writes to it. The path uses raw
//! register access instead of the live drivers and never resumes a HAL owner.

use super::reset::ResetControl;
use crate::process_io::{GpioPin, SafeLevels, CHIP_GPIO_PINS};
use esp_hal::gpio::OutputSignal;
use esp_hal::peripherals::GPIO;
use trust_platform_memory::reset::ResetCause;

/// The configured outputs and their safe levels. [`register_output`] records a pin once it has
/// become an output, so the fault path drives only pins it can drive.
static SAFE_LEVELS: SafeLevels = SafeLevels::new();

/// Records a configured output and its safe physical level, so [`emergency_halt`] can drive it
/// there: a GPIO line's output, or a `pwm` output, whose pin the fault path takes back from its LEDC
/// channel. Called after the pin has become an output, so a recorded pin is always drivable.
pub(crate) fn register_output(pin: GpioPin, safe_high: bool) {
    SAFE_LEVELS.record(pin, safe_high);
}

/// End this boot for good: mask interrupts, drive every configured output to its safe level, write
/// `text` as a panic's cause and location into the reset record, and reset the chip. It never
/// returns, and it never writes to the control link.
pub fn emergency_halt(text: &str) -> ! {
    disable_interrupts();
    drive_outputs_to_safe_levels();
    let mut reset = ResetControl::new();
    reset.write_reset_record(ResetCause::Panic, text);
    esp_hal::system::software_reset()
}

/// Clear `mstatus.MIE`, the global machine-mode interrupt enable.
fn disable_interrupts() {
    // SAFETY: `csrci mstatus, 8` clears only MIE (bit 3 of mstatus) and touches no memory. This
    // is the terminal boundary of a single-core firmware, so no suspended handler resumes.
    unsafe { core::arch::asm!("csrci mstatus, 8", options(nomem, nostack)) };
}

/// Latch every configured output at its safe level, route it to its latch and enable its driver,
/// through the bank-0 write-one registers and each pin's GPIO-matrix output selection
/// (specification 34, TARGET-08: each output's `[[io.safe_state]]` value, and de-energized without
/// one). An output that was never claimed is not touched.
fn drive_outputs_to_safe_levels() {
    let gpio = GPIO::regs();
    let (high, low) = SAFE_LEVELS.masks();
    // SAFETY: `out_w1ts` is write-one-to-set. Writing only the configured outputs whose safe level
    // is high sets their output latch and no other pin, and the register has no read-side effect.
    gpio.out_w1ts().write(|w| unsafe { w.bits(high) });
    // SAFETY: `out_w1tc` is write-one-to-clear. Writing only the configured outputs whose safe level
    // is low clears their output latch and no other pin, and the register has no read-side effect.
    gpio.out_w1tc().write(|w| unsafe { w.bits(low) });
    // A `pwm` output's pin carries its LEDC channel, which its latch does not reach until the pin
    // selects the latch again. A GPIO line's output selects it already, and selecting it again
    // changes nothing.
    for number in 0..CHIP_GPIO_PINS {
        if (high | low) & (1 << number) != 0 {
            // SAFETY: `FUNCn_OUT_SEL_CFG` of a configured output pin, n below the chip's 31 pins,
            // selects what drives the pin. Writing the GPIO selection (128, `OutputSignal::GPIO`)
            // with the other fields at their reset values makes the pin's latch, set above, drive
            // it, as esp-hal's `init_gpio` configures every GPIO line, and changes no other pin.
            gpio.func_out_sel_cfg(number as usize)
                .write(|w| unsafe { w.out_sel().bits(OutputSignal::GPIO as _) });
        }
    }
    // SAFETY: `enable_w1ts` is write-one-to-set. Writing only the configured output pins enables
    // their output driver, which is already latched at its safe level, and no other pin.
    gpio.enable_w1ts().write(|w| unsafe { w.bits(high | low) });
}

/// Forgets a configured output whose line was dropped with its program, so that [`emergency_halt`] no
/// longer drives it: the fault path drives the outputs of the program that runs (specification 34
/// STATE-08 and TARGET-08).
pub(crate) fn forget_output(pin: GpioPin) {
    SAFE_LEVELS.forget(pin);
}
