//! Bounded, nonallocating USART2 trace output on the ST-LINK virtual COM port.
use crate::console_budget::{ConsoleError, RecordBudget};
use core::fmt;
use cortex_m::peripheral::DWT;
use embedded_hal_nb::serial::Write as SerialWrite;
use stm32f4::stm32f401::USART2;
use stm32f4xx_hal::serial::{Rx, Tx};

/// USART2 TX owner with PA3 reserved for the board's matching receive connection.
/// `write_fmt` bounds the whole formatted record; `write_str` is one bounded record.
pub struct Console {
    tx: Tx<USART2>,
    _rx: Rx<USART2>,
    last_error: Option<ConsoleError>,
}
impl Console {
    pub(super) fn new(tx: Tx<USART2>, rx: Rx<USART2>) -> Self {
        Self {
            tx,
            _rx: rx,
            last_error: None,
        }
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
            match writer.console.tx.flush() {
                Ok(()) => return Ok(()),
                Err(nb::Error::WouldBlock) => cortex_m::asm::nop(),
                Err(nb::Error::Other(_)) => return writer.fail(ConsoleError::Peripheral),
            }
        }
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
                match self.console.tx.write(byte) {
                    Ok(()) => break,
                    Err(nb::Error::WouldBlock) => cortex_m::asm::nop(),
                    Err(nb::Error::Other(_)) => return self.fail(ConsoleError::Peripheral),
                }
            }
        }
        Ok(())
    }
}
