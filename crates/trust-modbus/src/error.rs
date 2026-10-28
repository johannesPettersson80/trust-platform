//! Why a request is not sent, or why a response is not taken.

use core::fmt;

use crate::pdu::{ExceptionCode, FunctionCode};

/// Why a request is refused before anything is sent, or why a response does not answer its
/// request. Each response error names the field that is wrong, with the value the request
/// expected and the value that came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The request's quantity is 0 or above its function's maximum (Modbus Application Protocol
    /// V1.1b3 §6.1 to §6.4, §6.11, §6.12).
    QuantityOutOfRange {
        /// The request's function.
        function: FunctionCode,
        /// The quantity of coils, inputs or registers the request names.
        quantity: u16,
        /// The function's greatest quantity.
        max: u16,
    },
    /// The request's coils, inputs or registers run past the last address, 0xFFFF: its address
    /// plus its quantity is above 65,536 (V1.1b3 §6).
    AddressRangeOverflow {
        /// The request's first address.
        address: u16,
        /// The request's quantity.
        quantity: u16,
    },
    /// Packed bits whose bytes are not one for every eight bits and one more for the rest.
    PackedBitsLength {
        /// How many bits.
        quantity: u16,
        /// How many bytes were given.
        len: usize,
    },
    /// Register bytes of an odd length, or of more than 65,535 registers.
    RegisterBytes {
        /// How many bytes were given.
        len: usize,
    },
    /// The buffer is shorter than the frame to be written into it.
    BufferTooSmall {
        /// The frame's length.
        needed: usize,
        /// The buffer's length.
        available: usize,
    },
    /// An RTU address from 248 to 255, which no server has (Modbus over Serial Line V1.02 §2.2).
    ReservedAddress {
        /// The address.
        address: u8,
    },
    /// A read to the RTU broadcast address 0: a broadcast is a write, which no server answers
    /// (V1.02 §2.1).
    BroadcastRead {
        /// The read's function.
        function: FunctionCode,
    },
    /// A response to the RTU broadcast address 0 was to be read: no server answers a broadcast
    /// (V1.02 §2.1).
    NoResponseToBroadcast,
    /// A baud rate of 0, which has no character time.
    ZeroBaudRate,
    /// The server answered with an exception response (V1.1b3 §7): it received the request and
    /// did not carry it out. The response itself is valid Modbus.
    Exception {
        /// The request's function.
        function: FunctionCode,
        /// The exception code the server sent.
        code: ExceptionCode,
    },
    /// The response's function code is neither the request's nor the request's plus 0x80.
    UnexpectedFunction {
        /// The request's function.
        expected: FunctionCode,
        /// The function code that came.
        actual: u8,
    },
    /// The byte count of a read response is not that of the requested quantity (V1.1b3 §6.1 to
    /// §6.4).
    ByteCountMismatch {
        /// The byte count of the requested quantity.
        expected: u8,
        /// The byte count that came.
        actual: u8,
    },
    /// The response PDU has more or fewer bytes than its function and the request call for.
    ResponseLength {
        /// The length the response should have.
        expected: usize,
        /// The length that came.
        actual: usize,
    },
    /// A write response echoes another address than the request's (V1.1b3 §6.5, §6.6, §6.11,
    /// §6.12).
    AddressMismatch {
        /// The request's address.
        expected: u16,
        /// The address that came.
        actual: u16,
    },
    /// A Write Single Coil or Write Single Register response echoes another value than the
    /// request's (V1.1b3 §6.5, §6.6).
    ValueMismatch {
        /// The request's value: 0xFF00 or 0x0000 for a coil.
        expected: u16,
        /// The value that came.
        actual: u16,
    },
    /// A Write Multiple Coils or Write Multiple Registers response echoes another quantity than
    /// the request's (V1.1b3 §6.11, §6.12).
    QuantityMismatch {
        /// The request's quantity.
        expected: u16,
        /// The quantity that came.
        actual: u16,
    },
    /// A received frame is shorter or longer than its transport allows: 8 to 260 bytes for Modbus
    /// TCP, 4 to 256 for RTU (V1.1b3 §4.1).
    FrameLength {
        /// The frame's length.
        len: usize,
        /// The least length.
        min: usize,
        /// The greatest length.
        max: usize,
    },
    /// The MBAP protocol identifier is not 0, the one of Modbus (Modbus Messaging on TCP/IP
    /// Implementation Guide V1.0b §3.1.3).
    ProtocolId {
        /// The protocol identifier that came.
        actual: u16,
    },
    /// The MBAP length is outside 2 to 254: the unit identifier and a PDU of 1 to 253 bytes.
    MbapLength {
        /// The length that came.
        length: u16,
    },
    /// The MBAP length does not count the bytes that follow it in the frame.
    MbapLengthMismatch {
        /// The length the header gives.
        length: u16,
        /// The bytes after the length field.
        following: usize,
    },
    /// The response's MBAP transaction identifier is not the request's (V1.0b §3.1.3).
    TransactionMismatch {
        /// The request's transaction identifier.
        expected: u16,
        /// The transaction identifier that came.
        actual: u16,
    },
    /// The response comes from another server address than the request went to (RTU, V1.02
    /// §2.3). Modbus TCP does not compare the unit (V1.0b §4.4.1.3).
    UnitMismatch {
        /// The unit or address the request went to.
        expected: u8,
        /// The unit or address that answered.
        actual: u8,
    },
    /// The CRC of an RTU frame does not match the frame's bytes (V1.02 §2.5.1.2).
    CrcMismatch {
        /// The CRC of the frame's address and PDU.
        computed: u16,
        /// The CRC the frame carries.
        received: u16,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::QuantityOutOfRange {
                function,
                quantity,
                max,
            } => write!(f, "the quantity {quantity} of {function} is not 1 to {max}"),
            Self::AddressRangeOverflow { address, quantity } => {
                write!(f, "{quantity} addresses from {address} run past 65535")
            }
            Self::PackedBitsLength { quantity, len } => {
                write!(f, "{quantity} packed bits do not take {len} bytes")
            }
            Self::RegisterBytes { len } => {
                write!(f, "{len} bytes are not whole registers up to 65535")
            }
            Self::BufferTooSmall { needed, available } => {
                write!(f, "the buffer holds {available} of the {needed} bytes")
            }
            Self::ReservedAddress { address } => {
                write!(f, "the RTU address {address} is reserved (248 to 255)")
            }
            Self::BroadcastRead { function } => {
                write!(f, "{function} cannot be broadcast: it reads")
            }
            Self::NoResponseToBroadcast => f.write_str("no server answers a broadcast"),
            Self::ZeroBaudRate => f.write_str("a baud rate of 0 has no character time"),
            Self::Exception { function, code } => {
                write!(f, "the server answered {function} with {code}")
            }
            Self::UnexpectedFunction { expected, actual } => {
                write!(f, "function code 0x{actual:02X} does not answer {expected}")
            }
            Self::ByteCountMismatch { expected, actual } => {
                write!(f, "the byte count is {actual}, not {expected}")
            }
            Self::ResponseLength { expected, actual } => {
                write!(f, "the response has {actual} bytes, not {expected}")
            }
            Self::AddressMismatch { expected, actual } => {
                write!(f, "the echoed address {actual} is not {expected}")
            }
            Self::ValueMismatch { expected, actual } => {
                write!(f, "the echoed value 0x{actual:04X} is not 0x{expected:04X}")
            }
            Self::QuantityMismatch { expected, actual } => {
                write!(f, "the echoed quantity {actual} is not {expected}")
            }
            Self::FrameLength { len, min, max } => {
                write!(f, "a frame of {len} bytes is not {min} to {max} bytes long")
            }
            Self::ProtocolId { actual } => {
                write!(f, "the MBAP protocol identifier is {actual}, not 0")
            }
            Self::MbapLength { length } => {
                write!(f, "the MBAP length {length} is not 2 to 254")
            }
            Self::MbapLengthMismatch { length, following } => {
                write!(f, "{following} bytes follow the MBAP length {length}")
            }
            Self::TransactionMismatch { expected, actual } => {
                write!(f, "the response is to transaction {actual}, not {expected}")
            }
            Self::UnitMismatch { expected, actual } => {
                write!(f, "the response comes from unit {actual}, not {expected}")
            }
            Self::CrcMismatch { computed, received } => {
                write!(f, "received CRC 0x{received:04X} is not 0x{computed:04X}")
            }
        }
    }
}

impl core::error::Error for Error {}
