//! Terminal fault boundary, usable without borrowing the normal board owners.
use crate::ResetCookie;
use core::sync::atomic::{AtomicBool, Ordering};
use stm32f4::stm32f401 as pac;

static CONSOLE_READY: AtomicBool = AtomicBool::new(false);
pub(super) fn console_ready() {
    CONSOLE_READY.store(true, Ordering::Relaxed);
}

/// Permanently stop execution, force PA5 low, record phase u16::MAX and emit a
/// bounded `B1,FAIL,<code>` record if UART setup completed. This never returns or
/// resumes any HAL owner. It continues feeding an already-running watchdog so a
/// fault cannot become an unreported automatic reset loop. No allocation occurs.
pub fn emergency_halt(code: &str) -> ! {
    cortex_m::interrupt::disable();
    // SAFETY: this is a terminal single-core abort boundary. Interrupts are now
    // disabled and this function never returns to the abandoned HAL owners.
    // Shared PAC register views use volatile interior access, not new owned
    // peripheral tokens. No DMA is configured by this board adapter.
    let (rcc, gpio, pwr, rtc, uart, watchdog) = unsafe {
        (
            &*pac::RCC::ptr(),
            &*pac::GPIOA::ptr(),
            &*pac::PWR::ptr(),
            &*pac::RTC::ptr(),
            &*pac::USART2::ptr(),
            &*pac::IWDG::ptr(),
        )
    };
    watchdog.kr().write(|w| w.key().feed());
    rcc.ahb1enr().modify(|_, w| w.gpioaen().set_bit());
    let _ = rcc.ahb1enr().read();
    gpio.bsrr().write(|w| w.br5().set_bit());
    gpio.moder().modify(|_, w| w.moder5().output());
    rcc.apb1enr().modify(|_, w| w.pwren().set_bit());
    let _ = rcc.apb1enr().read();
    pwr.cr().modify(|_, w| w.dbp().set_bit());
    // RM0368 section 5.4.1: read back DBP before accessing backup registers.
    let _ = pwr.cr().read();
    let cookie = ResetCookie { phase: u16::MAX }.words();
    rtc.bkp1r().write(|w| w.bkp().set(0));
    rtc.bkp0r().write(|w| w.bkp().set(cookie[0]));
    rtc.bkp1r().write(|w| w.bkp().set(cookie[1]));
    if CONSOLE_READY.load(Ordering::Relaxed) {
        for byte in b"B1,FAIL,"
            .iter()
            .copied()
            .chain(code.bytes().take(64).map(|b| {
                if b.is_ascii_alphanumeric() || b == b'_' {
                    b
                } else {
                    b'_'
                }
            }))
            .chain(b"\r\n".iter().copied())
        {
            watchdog.kr().write(|w| w.key().feed());
            if !write_byte(uart, byte) {
                break;
            }
        }
        for _ in 0..16_384 {
            if uart.sr().read().tc().bit_is_set() {
                break;
            }
            cortex_m::asm::nop();
        }
    }
    loop {
        watchdog.kr().write(|w| w.key().feed());
        cortex_m::asm::nop();
    }
}

fn write_byte(uart: &pac::usart1::RegisterBlock, byte: u8) -> bool {
    for _ in 0..16_384 {
        if uart.sr().read().txe().bit_is_set() {
            uart.dr().write(|w| w.dr().set(u16::from(byte)));
            return true;
        }
        cortex_m::asm::nop();
    }
    false
}
