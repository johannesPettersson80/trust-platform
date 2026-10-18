//! Terminal fault boundary: drive every configured output to its safe level, write the cause and
//! location into the persistent reset record, and reset the chip (design §1.1 and R4).
//!
//! The control link carries replies only, so a fault never writes to it. The path uses raw
//! register access instead of the live drivers and never resumes a HAL owner.

use super::reset::ResetControl;
use crate::process_io::GpioPin;
use core::sync::atomic::{AtomicU32, Ordering};
use esp_hal::peripherals::GPIO;
use trust_platform_memory::reset::ResetCause;

/// The configured output pins, one bit per GPIO number. [`register_output`] sets a bit once the
/// pin has become an output, so the fault path drives only pins it can drive.
static OUTPUTS: AtomicU32 = AtomicU32::new(0);

/// Records a configured output, so [`emergency_halt`] can drive it to its safe level. Called
/// after the pin has become an output, so a recorded pin is always drivable.
pub(crate) fn register_output(pin: GpioPin) {
    OUTPUTS.fetch_or(1 << pin.number(), Ordering::Relaxed);
}

/// End this boot for good: mask interrupts, drive every configured output low, write `text` as a
/// panic's cause and location into the reset record, and reset the chip. It never returns, and it
/// never writes to the control link.
pub fn emergency_halt(text: &str) -> ! {
    disable_interrupts();
    drive_outputs_low();
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

/// Latch every configured output low and enable its driver, through the bank-0 write-one
/// registers. Low is the de-energized safe level the M2E board drove, and an output that was
/// never claimed is not touched.
fn drive_outputs_low() {
    let gpio = GPIO::regs();
    let outputs = OUTPUTS.load(Ordering::Relaxed);
    // SAFETY: `out_w1tc` is write-one-to-clear. Writing only the configured output pins clears
    // their output latch and no other pin, and the register has no read-side effect.
    gpio.out_w1tc().write(|w| unsafe { w.bits(outputs) });
    // SAFETY: `enable_w1ts` is write-one-to-set. Writing only the configured output pins enables
    // their output driver, which is already latched low, and no other pin.
    gpio.enable_w1ts().write(|w| unsafe { w.bits(outputs) });
}
