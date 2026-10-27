//! The Modbus protocol of truST's Modbus drivers, `modbus-tcp` and `modbus-rtu` (specification 11,
//! built-in drivers 1 and 6; checklist S7 and S9), without `std` and without a heap, so that the
//! computer runtime and the chips' firmware send and check the same bytes. truST is the client.
//!
//! - [`pdu`]: the requests of function codes 1 to 6, 15 and 16, the responses to them and the
//!   exception responses (Modbus Application Protocol Specification V1.1b3).
//! - [`tcp`]: the MBAP header of Modbus TCP (Modbus Messaging on TCP/IP Implementation Guide
//!   V1.0b).
//! - [`rtu`]: the RTU frame with its CRC, and the silent intervals of the line (Modbus over Serial
//!   Line Specification and Implementation Guide V1.02).
//!
//! A driver builds a [`Request`], writes it into its buffer with [`tcp::encode_request`] or
//! [`rtu::encode_request`], sends the bytes, and hands the bytes that come back to
//! [`tcp::decode_response`] or [`rtu::decode_response`] with the same request. A request outside
//! the specification's limits is refused before a byte is written. A response that does not answer
//! the request is refused with an [`Error`] that names what is wrong, and an exception response is
//! [`Error::Exception`]. Nothing here opens a connection, waits or keeps time: the driver owns its
//! socket or serial port, its timeouts, and the silent intervals that [`rtu::silent_intervals`]
//! gives.
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod pdu;
pub mod rtu;
pub mod tcp;

pub use error::Error;
pub use pdu::{Bits, ExceptionCode, FunctionCode, Registers, Request, Response};
