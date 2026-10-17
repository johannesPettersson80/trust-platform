//! Terminal fault boundary, usable without borrowing the normal board owners.
use crate::{pac, pins::Pin, ResetCookie};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static CONSOLE_READY: AtomicBool = AtomicBool::new(false);
pub(super) fn console_ready() {
    CONSOLE_READY.store(true, Ordering::Relaxed);
}

/// Per port, the outputs the firmware drives and the level each takes on a fault: bits 0 to 15
/// mark the pins, bits 16 to 31 mark those whose safe level is high.
static OUTPUTS: [AtomicU32; 9] = [const { AtomicU32::new(0) }; 9];

/// Records a configured output, so that `emergency_halt` can put it in its safe state. Called
/// after the pin is an output, so a pin recorded here is always driven.
pub(super) fn register_output(pin: Pin, safe_high: bool) {
    let mask = u32::from(pin.mask());
    let word = if safe_high { mask | (mask << 16) } else { mask };
    if let Some(port) = OUTPUTS.get(pin.port().index()) {
        port.fetch_or(word, Ordering::Relaxed);
    }
}

/// Permanently stop execution, drive every configured output to its safe level, record phase
/// u16::MAX and emit a bounded `B1,FAIL,<code>` record if UART setup completed. This never
/// returns or resumes any HAL owner. It continues feeding an already-running watchdog so a
/// fault cannot become an unreported automatic reset loop. No allocation occurs.
pub fn emergency_halt(code: &str) -> ! {
    cortex_m::interrupt::disable();
    // SAFETY: this is a terminal single-core abort boundary. Interrupts are now
    // disabled and this function never returns to the abandoned owners. Shared PAC
    // register views use volatile interior access, not new owned peripheral tokens.
    // The nine GPIO views are used only for BSRR, whose writes touch only the pins
    // named in the write. No DMA is configured by this board adapter.
    let (rcc, pwr, rtc, uart, watchdog, ports) = unsafe {
        (
            &*pac::RCC::ptr(),
            &*pac::PWR::ptr(),
            &*pac::RTC::ptr(),
            &*pac::USART2::ptr(),
            &*pac::IWDG::ptr(),
            [
                (&*pac::GPIOA::ptr()).bsrr(),
                (&*pac::GPIOB::ptr()).bsrr(),
                (&*pac::GPIOC::ptr()).bsrr(),
                (&*pac::GPIOD::ptr()).bsrr(),
                (&*pac::GPIOE::ptr()).bsrr(),
                (&*pac::GPIOF::ptr()).bsrr(),
                (&*pac::GPIOG::ptr()).bsrr(),
                (&*pac::GPIOH::ptr()).bsrr(),
                (&*pac::GPIOI::ptr()).bsrr(),
            ],
        )
    };
    watchdog.kr().write(|w| w.key().feed());
    for (port, bsrr) in ports.iter().enumerate() {
        let word = OUTPUTS
            .get(port)
            .map_or(0, |outputs| outputs.load(Ordering::Relaxed));
        let outputs = word & 0xffff;
        let high = (word >> 16) & outputs;
        let low = outputs & !high;
        if outputs != 0 {
            bsrr.write(|w| {
                for n in 0..16_u8 {
                    if (high >> n) & 1 != 0 {
                        w.bs(n).set_bit();
                    }
                    if (low >> n) & 1 != 0 {
                        w.br(n).set_bit();
                    }
                }
                w
            });
        }
    }
    rcc.apb1enr().modify(|_, w| w.pwren().set_bit());
    let _ = rcc.apb1enr().read();
    pwr.cr().modify(|_, w| w.dbp().set_bit());
    // RM0090 section 5.4.1: read back DBP before accessing backup registers.
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
