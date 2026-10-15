//! Terminal fault boundary. It uses raw register access instead of the live drivers and keeps
//! MWDT0 fed, so a fault leaves the board halted with the output low instead of resetting it.
use super::clock::cycles_now;
use crate::{
    profile::{OUTPUT_GPIO, RECORD_DEADLINE_CYCLES},
    record::{fail_record, FAIL_RECORD_MAX},
};
use core::hint;
use esp_hal::peripherals::{GPIO, TIMG0, USB_DEVICE};

/// Write-protect key that unlocks the TIMG watchdog registers.
const WDT_WRITE_KEY: u32 = 0x50D8_3AA1;
/// Bit of GPIO18 in the bank-0 output and output-enable registers.
const OUTPUT_BIT: u32 = 1 << OUTPUT_GPIO;

/// Stop normal execution for good. Interrupts are masked, GPIO18 is driven low through its
/// output driver, a bounded `B1,FAIL,<code>` record is written if the host reads it, and MWDT0 is
/// fed forever. It never returns and never resumes a driver. The output is only configured as a
/// GPIO after `Board::take` has set GPIO18 up; a fault before that leaves the pad in its reset
/// state.
pub fn emergency_halt(message: &str) -> ! {
    disable_interrupts();
    feed_mwdt0();
    drive_output_low();
    let mut record = [0u8; FAIL_RECORD_MAX];
    let length = fail_record(message, &mut record);
    write_usb(&record[..length]);
    loop {
        feed_mwdt0();
        hint::spin_loop();
    }
}

/// Clear `mstatus.MIE`, the global machine-mode interrupt enable.
fn disable_interrupts() {
    // SAFETY: `csrci mstatus, 8` clears only MIE (bit 3 of mstatus) and touches no memory. This
    // is the terminal boundary of a single-core firmware, so no suspended handler resumes.
    unsafe { core::arch::asm!("csrci mstatus, 8", options(nomem, nostack)) };
}

/// Latch GPIO18 low, then enable its output driver, through the bank-0 write-one registers.
fn drive_output_low() {
    let gpio = GPIO::regs();
    // SAFETY: `out_w1tc` is write-one-to-clear. Writing only `OUTPUT_BIT` clears GPIO18's output
    // latch and no other pin, and the register has no read-side effect.
    gpio.out_w1tc().write(|w| unsafe { w.bits(OUTPUT_BIT) });
    // SAFETY: `enable_w1ts` is write-one-to-set. Writing only `OUTPUT_BIT` enables the output
    // driver of GPIO18, which is already latched low, and no other pin.
    gpio.enable_w1ts().write(|w| unsafe { w.bits(OUTPUT_BIT) });
}

/// Unlock MWDT0, feed it and lock it again, using the register sequence of `Wdt::feed`.
fn feed_mwdt0() {
    let timg = TIMG0::regs();
    // SAFETY: the value is the documented TIMG write-protect key for `wdt_wkey`, and the write
    // touches only the watchdog write-protect register.
    timg.wdtwprotect()
        .write(|w| unsafe { w.wdt_wkey().bits(WDT_WRITE_KEY) });
    // SAFETY: writing any value to `wdtfeed` restarts the MWDT0 counter and nothing else.
    timg.wdtfeed().write(|w| unsafe { w.bits(1) });
    // SAFETY: a zero key re-arms write protection on the watchdog registers.
    timg.wdtwprotect()
        .write(|w| unsafe { w.wdt_wkey().bits(0) });
}

/// Write the bytes through USB-Serial/JTAG EP1, feeding MWDT0 between bytes. The whole record
/// shares one cycle-counter deadline, and the rest of it is dropped when the host does not read.
fn write_usb(bytes: &[u8]) {
    let usb = USB_DEVICE::regs();
    let start = cycles_now();
    let expired = || cycles_now().wrapping_sub(start) >= RECORD_DEADLINE_CYCLES;
    for &byte in bytes {
        feed_mwdt0();
        while !usb.ep1_conf().read().serial_in_ep_data_free().bit_is_set() {
            if expired() {
                return;
            }
            hint::spin_loop();
        }
        // SAFETY: `rdwr_byte` is the full 8-bit EP1 data field, so every byte value is valid.
        usb.ep1().write(|w| unsafe { w.rdwr_byte().bits(byte) });
    }
    usb.ep1_conf().modify(|_, w| w.wr_done().set_bit());
    while (usb.ep1_conf().read().bits() & 0b011) == 0 {
        if expired() {
            return;
        }
        hint::spin_loop();
    }
}
