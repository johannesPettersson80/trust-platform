//! The `modbus-tcp` driver's parameters and its exchange with the process images (specification 11
//! §6.5, built-in driver 1), one implementation on every runtime (specification 34 ARCH-11). The
//! frames are those of the `trust-modbus` crate: the Modbus Application Protocol V1.1b3 and Modbus
//! Messaging on TCP/IP V1.0b. A chip reads the parameters from its record's section 8
//! (specification 34 §5.1.1), a computer from its `io.toml`, so both refuse and accept the same
//! files.
//!
//! [`ModbusTcpParams`] reads and checks the `params` table with the keys, defaults and aliases of
//! the computer's driver. [`ModbusExchange`] holds no socket and reads no clock: each runtime's
//! transport opens the connection, sends the frames the exchange gives it and hands it the bytes
//! that come back, with the time of a monotonic clock in milliseconds.
//!
//! A scan boundary is two calls, because the fixed scan-side deadline lies between them
//! (specification 11, "Modbus default-fault and deadline integrity"):
//!
//! - at the cycle start, [`ModbusExchange::request_inputs`] asks for a fresh input snapshot and
//!   gives the deadline; the caller drives the transport, or waits for the thread that drives it,
//!   until [`ModbusExchange::inputs_settled`] or the deadline, and then
//!   [`ModbusExchange::read_inputs`] copies the snapshot or applies `on_error`;
//! - after the tasks, [`ModbusExchange::write_outputs`] hands the output image off and gives the
//!   deadline; at [`ModbusExchange::outputs_settled`] or the deadline,
//!   [`ModbusExchange::output_result`] gives the result under `on_error`.
//!
//! The transport asks [`ModbusExchange::poll`] for its next step and reports what happened with
//! [`ModbusExchange::connected`], [`ModbusExchange::received`] and [`ModbusExchange::failed`].

mod exchange;
mod frame;
mod params;
mod plan;
mod point_map;
mod requests;

#[cfg(test)]
mod tests;

use alloc::format;
use alloc::string::String;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;

use crate::io_drivers::{ParamsError, Table};

pub use exchange::{ModbusExchange, ModbusPoll, RECONNECT_BACKOFF_MS, SCAN_DEADLINE_MS};
pub use params::{InputFunction, ModbusTcpAddress, ModbusTcpParams, OutputFunction};
pub use point_map::{Endian, InputPoint, OutputPoint, Point, PointType};

/// Why an input refresh or an output handoff failed: the text that the driver's health shows, and
/// the kind of error that it is at the scan boundary under `on_error = "fault"`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Failure {
    /// What kind of failure it is.
    kind: FailureKind,
    /// What failed, in words.
    text: String,
    /// The code of an exception response (V1.1b3 §7), for an exception.
    exception: Option<u8>,
}

/// The kinds of failure, which the scan boundary keeps apart (specification 11 §6.5: a Modbus
/// exception remains an `IoAddress` error and is never rewritten to a transport error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    /// The connection, a timeout or a response that fails a check: `IoTransport`. The exchange
    /// gives the connection up and waits [`RECONNECT_BACKOFF_MS`] before it connects again.
    Transport,
    /// An exception response: `IoAddress`. The response is valid Modbus, so the connection stays.
    Exception,
    /// A value that does not convert, or images that the parameters do not fit: `IoDriver`.
    Data,
}

impl Failure {
    /// A failure of the connection or of a response's checks.
    fn transport(text: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Transport,
            text: text.into(),
            exception: None,
        }
    }

    /// An exception response with the exception code `code`.
    fn exception(code: u8, text: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Exception,
            text: text.into(),
            exception: Some(code),
        }
    }

    /// A value or an image that does not fit.
    fn data(text: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Data,
            text: text.into(),
            exception: None,
        }
    }

    /// The error of the scan boundary.
    fn error(&self) -> RuntimeError {
        let text = SmolStr::new(&self.text);
        match self.kind {
            FailureKind::Transport => RuntimeError::IoTransport(text),
            FailureKind::Exception => RuntimeError::IoAddress(text),
            FailureKind::Data => RuntimeError::IoDriver(text),
        }
    }
}

/// The value that the word of `key` names, one of `words`, when the key is given. The word is read
/// as the computer's driver reads it: trimmed, in any case, and with `-` as `_`, so that
/// `"Big-Endian"` is `big_endian`. A refusal lists `what`, the words without their aliases.
fn word<T: Copy>(
    table: &Table<'_>,
    key: &str,
    words: &[(&str, T)],
    what: &str,
) -> Result<Option<T>, ParamsError> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|text| text.trim().to_ascii_lowercase().replace('-', "_"))
        .and_then(|text| {
            words
                .iter()
                .find(|(name, _)| *name == text)
                .map(|(_, choice)| *choice)
        })
        .map(Some)
        .ok_or_else(|| table.refuse(key, format!("must be {what}, not {value}")))
}
