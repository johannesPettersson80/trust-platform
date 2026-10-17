//! Bounded, nonallocating USART2 trace output on the chip's console pins. The pins are routed
//! by the pin driver and the register writes are the PAC's, so the driver that owns every GPIO
//! port also owns the console pins (the HAL's serial type would need its own pin tokens).
use super::{gpio::DigitalIo, BoardError};
use crate::{
    baud::{divisor, CONSOLE_BAUD},
    console_budget::{ConsoleError, RecordBudget},
    pac,
};
use core::fmt;
use cortex_m::peripheral::DWT;
use stm32f4xx_hal::rcc::Rcc;

/// USART2 transmitter on the chip's console pins, 230400 baud, 8N1. The receive pin is routed to
/// the USART but nothing reads it yet. `write_fmt` bounds the whole formatted record;
/// `write_str` is one bounded record.
pub struct Console {
    usart: pac::USART2,
    last_error: Option<ConsoleError>,
}

impl Console {
    pub(super) fn new(
        usart: pac::USART2,
        pins: &mut DigitalIo,
        rcc: &mut Rcc,
    ) -> Result<Self, BoardError> {
        let divisor = divisor(rcc.clocks.pclk1().raw(), CONSOLE_BAUD)
            .ok_or(BoardError::SerialConfiguration)?;
        pins.route_console();
        rcc.apb1enr().modify(|_, w| w.usart2en().set_bit());
        rcc.apb1rstr().modify(|_, w| w.usart2rst().set_bit());
        rcc.apb1rstr().modify(|_, w| w.usart2rst().clear_bit());
        usart.brr().write(|w| {
            w.div_mantissa()
                .set(divisor.mantissa)
                .div_fraction()
                .set(divisor.fraction)
        });
        // 8 data bits, no parity, 1 stop bit, no flow control: the reset values of CR2 and CR3.
        usart.cr2().reset();
        usart.cr3().reset();
        usart.cr1().write(|w| w.ue().set_bit().te().set_bit());
        Ok(Self {
            usart,
            last_error: None,
        })
    }

    /// Inspect the most recent formatting failure, cleared at the next record.
    pub fn last_error(&self) -> Option<ConsoleError> {
        self.last_error
    }

    /// Write and drain one record under a shared 512-byte, 100ms allowance.
    /// The callback runs once; all its `write_str` fragments share this transaction.
    pub fn write_record(
        &mut self,
        write: &mut dyn FnMut(&mut dyn fmt::Write) -> fmt::Result,
    ) -> fmt::Result {
        self.last_error = None;
        let mut writer = Record {
            console: self,
            budget: RecordBudget::new(DWT::cycle_count()),
        };
        write(&mut writer)?;
        loop {
            writer.check_time()?;
            if writer.console.drained() {
                return Ok(());
            }
            cortex_m::asm::nop();
        }
    }

    /// Hands one byte to the transmitter when it has room.
    fn put(&mut self, byte: u8) -> bool {
        if self.usart.sr().read().txe().bit_is_set() {
            self.usart.dr().write(|w| w.dr().set(u16::from(byte)));
            true
        } else {
            false
        }
    }

    /// Whether the last byte has left the shift register.
    fn drained(&self) -> bool {
        self.usart.sr().read().tc().bit_is_set()
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write_record(&mut |writer| writer.write_str(text))
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.write_record(&mut |writer| fmt::write(writer, args))
    }
}

struct Record<'a> {
    console: &'a mut Console,
    budget: RecordBudget,
}
impl Record<'_> {
    fn fail(&mut self, error: ConsoleError) -> fmt::Result {
        self.console.last_error = Some(error);
        Err(fmt::Error)
    }
    fn check_time(&mut self) -> fmt::Result {
        self.budget
            .check_time(DWT::cycle_count())
            .or_else(|error| self.fail(error))
    }
}
impl fmt::Write for Record<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if let Err(error) = self.budget.reserve(text.len()) {
            return self.fail(error);
        }
        for byte in text.bytes() {
            loop {
                self.check_time()?;
                if self.console.put(byte) {
                    break;
                }
                cortex_m::asm::nop();
            }
        }
        Ok(())
    }
}
