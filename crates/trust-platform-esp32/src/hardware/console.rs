//! Bounded, nonblocking USB-Serial/JTAG trace output. Each write is one record that must drain
//! within its shared byte and time allowance, or it fails.
use super::clock::now_us;
use crate::console_budget::{ConsoleError, RecordBudget};
use core::{fmt, hint};
use esp_hal::{usb::usb_serial_jtag::UsbSerialJtag, Blocking};

/// Native USB-Serial/JTAG transmitter. `write_fmt` bounds the whole formatted record, and
/// `write_str` is one bounded record.
pub struct Console {
    usb: UsbSerialJtag<'static, Blocking>,
    last_error: Option<ConsoleError>,
}

impl Console {
    pub(super) fn new(usb: UsbSerialJtag<'static, Blocking>) -> Self {
        Self {
            usb,
            last_error: None,
        }
    }

    /// The most recent record failure, cleared when the next record starts.
    pub fn last_error(&self) -> Option<ConsoleError> {
        self.last_error
    }

    /// Write and drain one record under a shared 512-byte, 100 ms allowance. The callback runs
    /// once, all of its fragments share this record, and the record is flushed when it ends.
    pub fn write_record(
        &mut self,
        write: &mut dyn FnMut(&mut dyn fmt::Write) -> fmt::Result,
    ) -> fmt::Result {
        self.last_error = None;
        let mut record = Record {
            console: self,
            budget: RecordBudget::new(now_us()),
        };
        write(&mut record)?;
        loop {
            record.check_time()?;
            if record.console.usb.flush_tx_nb().is_ok() {
                return Ok(());
            }
            hint::spin_loop();
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write_record(&mut |record| record.write_str(text))
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.write_record(&mut |record| fmt::write(record, args))
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
        match self.budget.check_time(now_us()) {
            Ok(()) => Ok(()),
            Err(error) => self.fail(error),
        }
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
                // The driver's error type is `Infallible`, so an `Err` here means "FIFO full".
                if self.console.usb.write_byte_nb(byte).is_ok() {
                    break;
                }
                hint::spin_loop();
            }
        }
        Ok(())
    }
}
