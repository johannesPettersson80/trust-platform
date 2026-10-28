//! A chip's USB serial port as a control stream (specification 34, PORT-05, the client's rules).
//!
//! [`SerialStream`] sets the port up the way the bench does, so that opening it does not reset the
//! chip, and it reports a port that went away plainly. It runs over any [`SerialLine`]: the
//! operating system's port ([`open`]), or a test's.

use std::cell::Cell;
use std::fmt;
use std::io::{self, Read, Write};
use std::time::Duration;

use crate::transport::ByteStream;

/// The longest frame of a chip's USB link in either direction, in bytes without the line end
/// (specification 34, PORT-05): the same on every chip.
pub const SERIAL_FRAME_BYTES: usize = 8_192;

/// The baud rate the port is opened at. A USB serial port ignores it (specification 34, PORT-05).
const BAUD_RATE: u32 = 115_200;

/// What the client needs of a serial port: bytes in and out, the two modem lines that a chip's USB
/// controller watches, the bytes the operating system holds, and the wait.
pub trait SerialLine: Read + Write + Send {
    /// Sets the DTR line on (`true`) or off.
    ///
    /// # Errors
    /// The port refuses the setting or has gone away.
    fn set_dtr(&mut self, on: bool) -> io::Result<()>;

    /// Sets the RTS line on (`true`) or off.
    ///
    /// # Errors
    /// The port refuses the setting or has gone away.
    fn set_rts(&mut self, on: bool) -> io::Result<()>;

    /// Discards the bytes that the operating system holds for the port and that nobody has read.
    ///
    /// # Errors
    /// The port refuses the request or has gone away.
    fn discard_input(&mut self) -> io::Result<()>;

    /// Bounds every following read and write by `timeout`.
    ///
    /// # Errors
    /// The port refuses the setting or has gone away.
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()>;
}

/// A chip's serial port as a [`ByteStream`] with the frame of a chip's USB link
/// ([`SERIAL_FRAME_BYTES`]).
///
/// A read or write that fails for any reason but the wait running out, and a read that returns no
/// bytes, mean that the port went away; they fail with the plain text that specification 34,
/// PORT-05 gives for it.
pub struct SerialStream<L> {
    line: L,
    name: String,
    /// The wait the caller asked for; [`ByteStream::set_timeout`] takes `&self`.
    timeout: Cell<Duration>,
    /// The wait the line has been given.
    applied: Duration,
}

impl<L: SerialLine> SerialStream<L> {
    /// Sets `line`, the port `name`, up for a control client: DTR on and then RTS off, as the
    /// bench does and nothing else, so that opening the port does not reset a chip whose USB
    /// controller resets on the sequence of levels that its boot loader's flash tool uses; then it
    /// discards the bytes the operating system holds from an earlier session, and bounds every
    /// read and write by `timeout` (specification 34, PORT-05, "The client: opening the port").
    ///
    /// # Errors
    /// A setting fails. It is reported, never ignored.
    pub fn new(mut line: L, name: &str, timeout: Duration) -> io::Result<Self> {
        line.set_dtr(true)?;
        line.set_rts(false)?;
        line.discard_input()?;
        line.set_timeout(timeout)?;
        Ok(Self {
            line,
            name: name.to_string(),
            timeout: Cell::new(timeout),
            applied: timeout,
        })
    }

    /// Gives the line the wait that the caller last set.
    fn apply_timeout(&mut self) -> io::Result<()> {
        let wanted = self.timeout.get();
        if wanted != self.applied {
            self.line
                .set_timeout(wanted)
                .map_err(|error| self.link_error(error))?;
            self.applied = wanted;
        }
        Ok(())
    }

    /// The error for a failed read or write: the wait running out stays a timeout that names the
    /// wait, and any other failure means that the port went away.
    fn link_error(&self, error: io::Error) -> io::Error {
        match error.kind() {
            io::ErrorKind::Interrupted => error,
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "no response from serial port {} within {} ms",
                    self.name,
                    self.timeout.get().as_millis()
                ),
            ),
            _ => self.went_away(&error.to_string()),
        }
    }

    /// The plain report of a port that went away: a chip that resets, loses power or is
    /// unplugged is enumerated again, often under another port name.
    fn went_away(&self, reason: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!(
                "serial port {} went away ({reason}): the chip reset, lost power or was \
                 unplugged; its port returns when the chip has started again, so run the command \
                 again",
                self.name
            ),
        )
    }
}

impl<L> fmt::Debug for SerialStream<L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SerialStream")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<L: SerialLine> Read for SerialStream<L> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.apply_timeout()?;
        match self.line.read(buf) {
            // A serial port waits for a byte or the end of the wait; it never ends the stream.
            Ok(0) if !buf.is_empty() => Err(self.went_away("the port gave no bytes")),
            Ok(read) => Ok(read),
            Err(error) => Err(self.link_error(error)),
        }
    }
}

impl<L: SerialLine> Write for SerialStream<L> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.apply_timeout()?;
        self.line.write(buf).map_err(|error| self.link_error(error))
    }

    /// Does nothing: a write hands its bytes to the operating system's driver, and the wait for the
    /// reply starts there. Draining the driver's queue would wait without a bound for a chip that
    /// does not read.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<L: SerialLine> ByteStream for SerialStream<L> {
    fn set_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.timeout.set(timeout);
        Ok(())
    }

    fn frame_bytes(&self) -> Option<usize> {
        Some(SERIAL_FRAME_BYTES)
    }
}

/// The operating system's serial port, through the `serialport` crate.
struct Port(Box<dyn serialport::SerialPort>);

impl Read for Port {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Port {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl SerialLine for Port {
    fn set_dtr(&mut self, on: bool) -> io::Result<()> {
        self.0
            .write_data_terminal_ready(on)
            .map_err(io::Error::from)
    }

    fn set_rts(&mut self, on: bool) -> io::Result<()> {
        self.0.write_request_to_send(on).map_err(io::Error::from)
    }

    fn discard_input(&mut self) -> io::Result<()> {
        self.0
            .clear(serialport::ClearBuffer::Input)
            .map_err(io::Error::from)
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.0.set_timeout(timeout).map_err(io::Error::from)
    }
}

/// Opens the operating system's serial port `name` for a control client: 115,200 baud, 8 data
/// bits, no parity, one stop bit and no flow control, then the set-up of [`SerialStream::new`].
/// Every read and write waits at most `timeout`.
///
/// # Errors
/// The port cannot be opened or set up: it does not exist, belongs to another program or user, or
/// refuses a setting.
pub(crate) fn open(name: &str, timeout: Duration) -> io::Result<Box<dyn ByteStream>> {
    let port = serialport::new(name, BAUD_RATE)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(timeout)
        .dtr_on_open(true)
        .open()
        .map_err(io::Error::from)?;
    Ok(Box::new(SerialStream::new(Port(port), name, timeout)?))
}
