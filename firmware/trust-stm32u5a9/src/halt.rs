//! The terminal fault path (specification 34, R4; design §1.1): every configured output to its safe
//! level, the cause and the panic's location into the reset record, then a reset. It never writes to
//! the control link, never allocates and never formats more than the record keeps.
use core::fmt::Write;

use cortex_m::peripheral::SCB;
use stm32u5::stm32u5a9 as pac;
use trust_platform_memory::reset::{ResetCause, TEXT_BYTES};
use trust_platform_stm32u5::gpio::drive_outputs_to_safe_levels;
use trust_platform_stm32u5::reset::ResetControl;

/// Ends this boot for good with `text` as its cause: masks the interrupts, drives every output the
/// program claimed to its safe level, writes the cause into the reset record and resets the chip.
pub fn emergency_halt(text: &str) -> ! {
    cortex_m::interrupt::disable();
    drive_outputs_to_safe_levels();
    // SAFETY: the boot is over, so no other owner of the RCC, PWR and TAMP blocks runs again: the fault
    // path takes them for the reset record and the reset that follows. The record is written through
    // the backup registers only, which the reset keeps.
    let peripherals = unsafe { pac::Peripherals::steal() };
    let mut reset = ResetControl::take(&peripherals.RCC, &peripherals.PWR, peripherals.TAMP);
    reset.write_reset_record(ResetCause::Panic, text);
    SCB::sys_reset()
}

/// The panic's location is kept in the record, as far as its text holds it.
const LOCATION_BYTES: usize = TEXT_BYTES;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    // The message is never formatted: the location alone fits the record, and a location that does not
    // fit reads as `panic`, never as a partial path.
    let mut record = LocationRecord {
        bytes: [0; LOCATION_BYTES],
        length: 0,
    };
    let written = match info.location() {
        Some(location) => write!(record, "{}:{}", location.file(), location.line()),
        None => write!(record, "panic"),
    };
    let text = if written.is_ok() {
        core::str::from_utf8(&record.bytes[..record.length]).unwrap_or("panic")
    } else {
        "panic"
    };
    emergency_halt(text)
}

/// A fixed buffer the panic location is formatted into without allocating.
struct LocationRecord {
    bytes: [u8; LOCATION_BYTES],
    length: usize,
}

impl core::fmt::Write for LocationRecord {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let end = self
            .length
            .checked_add(text.len())
            .ok_or(core::fmt::Error)?;
        let target = self
            .bytes
            .get_mut(self.length..end)
            .ok_or(core::fmt::Error)?;
        target.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
