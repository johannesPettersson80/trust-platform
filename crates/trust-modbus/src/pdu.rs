//! The protocol data unit: the requests of function codes 1 to 6, 15 and 16, the responses to them
//! and the exception responses (Modbus Application Protocol Specification V1.1b3 §4.1, §6, §7).
//!
//! A [`Request`] holds its function's fields. [`encode_request`] checks it against the limits of
//! V1.1b3 §6 and writes it. [`decode_response`] takes a response only when it answers that
//! request: the request's function code, a byte count of exactly the requested quantity and that
//! many bytes, and for a write the echo of the request's address and its value or quantity. The
//! request's function code plus 0x80 and an exception code is an exception response, which
//! [`decode_response`] gives as [`Error::Exception`].
//!
//! Addresses are those of the PDU, counted from 0: the specification's coil 20 is address 19
//! (V1.1b3 §4.4). Words go high byte first (§4.2). Coils and discrete inputs go one per bit, the
//! first in the least significant bit of the first byte (§6.1, §6.2, §6.11).

use core::fmt;

use crate::Error;

/// The greatest PDU, in bytes: the 256 bytes of an RTU frame less its address and its CRC
/// (V1.1b3 §4.1).
pub const MAX_PDU_LEN: usize = 253;

/// One past the last address: coils, inputs and registers have the addresses 0x0000 to 0xFFFF
/// (V1.1b3 §6, "Starting Address").
const ADDRESS_END: u32 = 65_536;
/// What turns a function code into the function code of its exception response (V1.1b3 §7).
const EXCEPTION_FLAG: u8 = 0x80;
/// The value of a Write Single Coil request that sets the coil ON (V1.1b3 §6.5).
const COIL_ON: u16 = 0xFF00;
/// The value of a Write Single Coil request that sets the coil OFF (V1.1b3 §6.5).
const COIL_OFF: u16 = 0x0000;
/// The length of a request of codes 1 to 6, and of the response to a write: the function code and
/// two words (V1.1b3 §6).
const TWO_WORDS_LEN: u16 = 5;
/// The bytes of a write-multiple request before its values: the function code, the address, the
/// quantity and the byte count (V1.1b3 §6.11, §6.12).
const MULTIPLE_HEADER_LEN: u16 = 6;
/// The bytes of a read response before its data: the function code and the byte count (V1.1b3
/// §6.1 to §6.4).
const READ_HEADER_LEN: usize = 2;
/// The length of an exception response: the function code plus 0x80, and the exception code
/// (V1.1b3 §7).
const EXCEPTION_LEN: usize = 2;

/// A function code of truST's Modbus drivers (V1.1b3 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionCode {
    /// 0x01 Read Coils (§6.1).
    ReadCoils,
    /// 0x02 Read Discrete Inputs (§6.2).
    ReadDiscreteInputs,
    /// 0x03 Read Holding Registers (§6.3).
    ReadHoldingRegisters,
    /// 0x04 Read Input Registers (§6.4).
    ReadInputRegisters,
    /// 0x05 Write Single Coil (§6.5).
    WriteSingleCoil,
    /// 0x06 Write Single Register (§6.6).
    WriteSingleRegister,
    /// 0x0F Write Multiple Coils (§6.11).
    WriteMultipleCoils,
    /// 0x10 Write Multiple Registers (§6.12).
    WriteMultipleRegisters,
}

impl FunctionCode {
    /// The function code on the wire.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::ReadCoils => 0x01,
            Self::ReadDiscreteInputs => 0x02,
            Self::ReadHoldingRegisters => 0x03,
            Self::ReadInputRegisters => 0x04,
            Self::WriteSingleCoil => 0x05,
            Self::WriteSingleRegister => 0x06,
            Self::WriteMultipleCoils => 0x0F,
            Self::WriteMultipleRegisters => 0x10,
        }
    }

    /// The greatest quantity of one request of this function: 2,000 coils or inputs for codes 1
    /// and 2, 125 registers for codes 3 and 4, 1,968 coils for code 15 and 123 registers for
    /// code 16 (V1.1b3 §6.1 to §6.4, §6.11, §6.12); 1 for codes 5 and 6, which write one coil or
    /// register.
    #[must_use]
    pub const fn max_quantity(self) -> u16 {
        match self {
            Self::ReadCoils | Self::ReadDiscreteInputs => 2000,
            Self::ReadHoldingRegisters | Self::ReadInputRegisters => 125,
            Self::WriteSingleCoil | Self::WriteSingleRegister => 1,
            Self::WriteMultipleCoils => 1968,
            Self::WriteMultipleRegisters => 123,
        }
    }

    /// Whether the function writes: only a write may go to every server at once, as an RTU
    /// broadcast.
    pub(crate) const fn is_write(self) -> bool {
        matches!(
            self,
            Self::WriteSingleCoil
                | Self::WriteSingleRegister
                | Self::WriteMultipleCoils
                | Self::WriteMultipleRegisters
        )
    }

    /// The function's name, as the title of its section of V1.1b3 §6 gives it.
    const fn name(self) -> &'static str {
        match self {
            Self::ReadCoils => "Read Coils",
            Self::ReadDiscreteInputs => "Read Discrete Inputs",
            Self::ReadHoldingRegisters => "Read Holding Registers",
            Self::ReadInputRegisters => "Read Input Registers",
            Self::WriteSingleCoil => "Write Single Coil",
            Self::WriteSingleRegister => "Write Single Register",
            Self::WriteMultipleCoils => "Write Multiple Coils",
            Self::WriteMultipleRegisters => "Write Multiple Registers",
        }
    }
}

impl fmt::Display for FunctionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:02X} {}", self.code(), self.name())
    }
}

/// The exception code of an exception response (V1.1b3 §7, table "MODBUS Exception Codes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExceptionCode {
    /// 0x01 ILLEGAL FUNCTION: the server does not allow this function.
    IllegalFunction,
    /// 0x02 ILLEGAL DATA ADDRESS: the address, or the address with the quantity, is not one the
    /// server has.
    IllegalDataAddress,
    /// 0x03 ILLEGAL DATA VALUE: a value of the request is not allowed, such as a wrong implied
    /// length.
    IllegalDataValue,
    /// 0x04 SERVER DEVICE FAILURE: an unrecoverable error while the server carried out the
    /// request.
    ServerDeviceFailure,
    /// 0x05 ACKNOWLEDGE: the server accepted the request and needs a long time to carry it out.
    Acknowledge,
    /// 0x06 SERVER DEVICE BUSY: the server is busy with a long command; ask again later.
    ServerDeviceBusy,
    /// 0x08 MEMORY PARITY ERROR: the server found a parity error in the memory of its file
    /// records.
    MemoryParityError,
    /// 0x0A GATEWAY PATH UNAVAILABLE: a gateway has no path to the target server.
    GatewayPathUnavailable,
    /// 0x0B GATEWAY TARGET DEVICE FAILED TO RESPOND: no answer came to a gateway from its target.
    GatewayTargetDeviceFailedToRespond,
    /// A code that V1.1b3 does not list, as the server sent it.
    Other(u8),
}

impl ExceptionCode {
    /// The exception of a code on the wire: one of the nine that V1.1b3 §7 lists, or
    /// [`ExceptionCode::Other`].
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x01 => Self::IllegalFunction,
            0x02 => Self::IllegalDataAddress,
            0x03 => Self::IllegalDataValue,
            0x04 => Self::ServerDeviceFailure,
            0x05 => Self::Acknowledge,
            0x06 => Self::ServerDeviceBusy,
            0x08 => Self::MemoryParityError,
            0x0A => Self::GatewayPathUnavailable,
            0x0B => Self::GatewayTargetDeviceFailedToRespond,
            other => Self::Other(other),
        }
    }

    /// The exception code on the wire.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::IllegalFunction => 0x01,
            Self::IllegalDataAddress => 0x02,
            Self::IllegalDataValue => 0x03,
            Self::ServerDeviceFailure => 0x04,
            Self::Acknowledge => 0x05,
            Self::ServerDeviceBusy => 0x06,
            Self::MemoryParityError => 0x08,
            Self::GatewayPathUnavailable => 0x0A,
            Self::GatewayTargetDeviceFailedToRespond => 0x0B,
            Self::Other(code) => code,
        }
    }

    /// The name that V1.1b3 §7 gives the code, such as `ILLEGAL DATA ADDRESS`; `None` for a code
    /// it does not list.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::IllegalFunction => Some("ILLEGAL FUNCTION"),
            Self::IllegalDataAddress => Some("ILLEGAL DATA ADDRESS"),
            Self::IllegalDataValue => Some("ILLEGAL DATA VALUE"),
            Self::ServerDeviceFailure => Some("SERVER DEVICE FAILURE"),
            Self::Acknowledge => Some("ACKNOWLEDGE"),
            Self::ServerDeviceBusy => Some("SERVER DEVICE BUSY"),
            Self::MemoryParityError => Some("MEMORY PARITY ERROR"),
            Self::GatewayPathUnavailable => Some("GATEWAY PATH UNAVAILABLE"),
            Self::GatewayTargetDeviceFailedToRespond => {
                Some("GATEWAY TARGET DEVICE FAILED TO RESPOND")
            }
            Self::Other(_) => None,
        }
    }
}

impl fmt::Display for ExceptionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code();
        if let Some(name) = self.name() {
            write!(f, "exception code {code} ({name})")
        } else {
            write!(f, "exception code {code}, which V1.1b3 does not list")
        }
    }
}

/// Coils or discrete inputs as their PDUs carry them: one per bit, the first in the least
/// significant bit of the first byte, the next ones toward the high end of the byte and then on in
/// the next bytes (V1.1b3 §6.1, §6.2, §6.11). The bits past the quantity in the last byte belong to
/// no coil: [`Bits::get`] does not read them, and [`encode_request`] sends them as 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bits<'a> {
    quantity: u16,
    packed: &'a [u8],
}

impl<'a> Bits<'a> {
    /// `quantity` bits, packed in `packed`, which holds exactly one byte for every eight bits and
    /// one more for the rest.
    ///
    /// # Errors
    ///
    /// [`Error::PackedBitsLength`] when `packed` holds another number of bytes.
    pub fn new(quantity: u16, packed: &'a [u8]) -> Result<Self, Error> {
        if packed.len() == usize::from(quantity.div_ceil(8)) {
            Ok(Self { quantity, packed })
        } else {
            Err(Error::PackedBitsLength {
                quantity,
                len: packed.len(),
            })
        }
    }

    /// How many coils or inputs there are.
    #[must_use]
    pub const fn quantity(&self) -> u16 {
        self.quantity
    }

    /// The packed bytes, as given or as received.
    #[must_use]
    pub const fn packed(&self) -> &'a [u8] {
        self.packed
    }

    /// The state of the coil or input `index`, counted from 0 at the first: `true` for ON; `None`
    /// at or past the quantity.
    #[must_use]
    pub fn get(&self, index: u16) -> Option<bool> {
        if index >= self.quantity {
            return None;
        }
        let byte = *self.packed.get(usize::from(index / 8))?;
        Some((byte >> (index % 8)) & 1 == 1)
    }
}

/// Registers as their PDUs carry them: two bytes each, the high byte first (V1.1b3 §4.2, §6.3,
/// §6.4, §6.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers<'a> {
    quantity: u16,
    bytes: &'a [u8],
}

impl<'a> Registers<'a> {
    /// The registers in `bytes`, two bytes each, the high byte first.
    ///
    /// # Errors
    ///
    /// [`Error::RegisterBytes`] when `bytes` has an odd length or holds more than 65,535
    /// registers.
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        match u16::try_from(bytes.len() / 2) {
            Ok(quantity) if bytes.len().is_multiple_of(2) => Ok(Self { quantity, bytes }),
            _ => Err(Error::RegisterBytes { len: bytes.len() }),
        }
    }

    /// How many registers there are.
    #[must_use]
    pub const fn quantity(&self) -> u16 {
        self.quantity
    }

    /// The registers' bytes, the high byte of each first, as given or as received.
    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// The value of register `index`, counted from 0 at the first; `None` at or past the quantity.
    #[must_use]
    pub fn get(&self, index: u16) -> Option<u16> {
        let (pairs, _) = self.bytes.as_chunks::<2>();
        pairs
            .get(usize::from(index))
            .copied()
            .map(u16::from_be_bytes)
    }
}

/// A client's request: its function and the fields of its request PDU (V1.1b3 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request<'a> {
    /// Read Coils (§6.1): 1 to 2,000 coils.
    ReadCoils {
        /// The first coil's address.
        address: u16,
        /// How many coils.
        quantity: u16,
    },
    /// Read Discrete Inputs (§6.2): 1 to 2,000 inputs.
    ReadDiscreteInputs {
        /// The first input's address.
        address: u16,
        /// How many inputs.
        quantity: u16,
    },
    /// Read Holding Registers (§6.3): 1 to 125 registers.
    ReadHoldingRegisters {
        /// The first register's address.
        address: u16,
        /// How many registers.
        quantity: u16,
    },
    /// Read Input Registers (§6.4): 1 to 125 registers.
    ReadInputRegisters {
        /// The first register's address.
        address: u16,
        /// How many registers.
        quantity: u16,
    },
    /// Write Single Coil (§6.5).
    WriteSingleCoil {
        /// The coil's address.
        address: u16,
        /// `true` sets the coil ON, sent as 0xFF00; `false` sets it OFF, sent as 0x0000.
        value: bool,
    },
    /// Write Single Register (§6.6).
    WriteSingleRegister {
        /// The register's address.
        address: u16,
        /// The value to write.
        value: u16,
    },
    /// Write Multiple Coils (§6.11): 1 to 1,968 coils.
    WriteMultipleCoils {
        /// The first coil's address.
        address: u16,
        /// The coils' states, as many as the request writes.
        values: Bits<'a>,
    },
    /// Write Multiple Registers (§6.12): 1 to 123 registers.
    WriteMultipleRegisters {
        /// The first register's address.
        address: u16,
        /// The registers' values, as many as the request writes.
        values: Registers<'a>,
    },
}

impl Request<'_> {
    /// The request's function.
    #[must_use]
    pub const fn function(&self) -> FunctionCode {
        match self {
            Self::ReadCoils { .. } => FunctionCode::ReadCoils,
            Self::ReadDiscreteInputs { .. } => FunctionCode::ReadDiscreteInputs,
            Self::ReadHoldingRegisters { .. } => FunctionCode::ReadHoldingRegisters,
            Self::ReadInputRegisters { .. } => FunctionCode::ReadInputRegisters,
            Self::WriteSingleCoil { .. } => FunctionCode::WriteSingleCoil,
            Self::WriteSingleRegister { .. } => FunctionCode::WriteSingleRegister,
            Self::WriteMultipleCoils { .. } => FunctionCode::WriteMultipleCoils,
            Self::WriteMultipleRegisters { .. } => FunctionCode::WriteMultipleRegisters,
        }
    }

    /// Checks the request against the limits of V1.1b3 §6, as [`encode_request`] does before it
    /// writes a byte: a quantity from 1 to [`FunctionCode::max_quantity`], and every address it
    /// reads or writes within 0x0000 to 0xFFFF.
    ///
    /// # Errors
    ///
    /// [`Error::QuantityOutOfRange`] when the quantity is 0 or above its function's maximum, and
    /// [`Error::AddressRangeOverflow`] when the address plus the quantity is above 65,536.
    pub fn validate(&self) -> Result<(), Error> {
        self.byte_count().map(|_| ())
    }

    /// The address of the first coil, input or register that the request reads or writes.
    const fn address(&self) -> u16 {
        match self {
            Self::ReadCoils { address, .. }
            | Self::ReadDiscreteInputs { address, .. }
            | Self::ReadHoldingRegisters { address, .. }
            | Self::ReadInputRegisters { address, .. }
            | Self::WriteSingleCoil { address, .. }
            | Self::WriteSingleRegister { address, .. }
            | Self::WriteMultipleCoils { address, .. }
            | Self::WriteMultipleRegisters { address, .. } => *address,
        }
    }

    /// How many coils, inputs or registers the request reads or writes: 1 for codes 5 and 6.
    const fn quantity(&self) -> u16 {
        match self {
            Self::ReadCoils { quantity, .. }
            | Self::ReadDiscreteInputs { quantity, .. }
            | Self::ReadHoldingRegisters { quantity, .. }
            | Self::ReadInputRegisters { quantity, .. } => *quantity,
            Self::WriteSingleCoil { .. } | Self::WriteSingleRegister { .. } => 1,
            Self::WriteMultipleCoils { values, .. } => values.quantity(),
            Self::WriteMultipleRegisters { values, .. } => values.quantity(),
        }
    }

    /// Checks the request as [`Request::validate`] does, and gives the byte count of its data:
    /// the count that a request of code 15 or 16 carries, or that the response to a request of
    /// codes 1 to 4 must carry; 0 for codes 5 and 6.
    fn byte_count(&self) -> Result<u8, Error> {
        let function = self.function();
        let quantity = self.quantity();
        let max = function.max_quantity();
        if !(1..=max).contains(&quantity) {
            return Err(Error::QuantityOutOfRange {
                function,
                quantity,
                max,
            });
        }
        let address = self.address();
        if u32::from(address) + u32::from(quantity) > ADDRESS_END {
            return Err(Error::AddressRangeOverflow { address, quantity });
        }
        let bytes = match self {
            Self::ReadCoils { .. }
            | Self::ReadDiscreteInputs { .. }
            | Self::WriteMultipleCoils { .. } => quantity.div_ceil(8),
            Self::ReadHoldingRegisters { .. }
            | Self::ReadInputRegisters { .. }
            | Self::WriteMultipleRegisters { .. } => quantity * 2,
            Self::WriteSingleCoil { .. } | Self::WriteSingleRegister { .. } => 0,
        };
        // Within the maxima the data is at most 250 bytes (2,000 coils or 125 registers), so its
        // count fits the one byte that the PDU gives it.
        u8::try_from(bytes).map_err(|_| Error::QuantityOutOfRange {
            function,
            quantity,
            max,
        })
    }
}

/// What a response gives when it answers its request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response<'a> {
    /// The coils or discrete inputs that Read Coils or Read Discrete Inputs read, exactly as many
    /// as the request asked for.
    Bits(Bits<'a>),
    /// The registers that Read Holding Registers or Read Input Registers read, exactly as many as
    /// the request asked for.
    Registers(Registers<'a>),
    /// The answer to a write: the server echoed the request's address and its value (codes 5 and
    /// 6) or quantity (codes 15 and 16).
    Written,
}

/// Writes `request` at the start of `out` and gives its length: the function code and then the
/// request's fields in the order of V1.1b3 §6, each word high byte first. Write Multiple Coils
/// sends the bits past its quantity in the last byte as 0 (§6.11: "Unused bits in the last data
/// byte should be zero-filled").
///
/// # Errors
///
/// The errors of [`Request::validate`], and [`Error::BufferTooSmall`] when `out` is shorter than
/// the request. A refused request writes nothing.
pub fn encode_request(request: &Request<'_>, out: &mut [u8]) -> Result<usize, Error> {
    let byte_count = request.byte_count()?;
    let len = usize::from(encoded_len(request, byte_count));
    let available = out.len();
    let pdu = out.get_mut(..len).ok_or(Error::BufferTooSmall {
        needed: len,
        available,
    })?;
    let mut writer = Writer { out: pdu, len: 0 };
    writer.byte(request.function().code());
    writer.word(request.address());
    match *request {
        Request::ReadCoils { quantity, .. }
        | Request::ReadDiscreteInputs { quantity, .. }
        | Request::ReadHoldingRegisters { quantity, .. }
        | Request::ReadInputRegisters { quantity, .. } => writer.word(quantity),
        Request::WriteSingleCoil { value, .. } => writer.word(coil_word(value)),
        Request::WriteSingleRegister { value, .. } => writer.word(value),
        Request::WriteMultipleCoils { values, .. } => {
            writer.word(values.quantity());
            writer.byte(byte_count);
            if let Some((&last, rest)) = values.packed().split_last() {
                writer.bytes(rest);
                writer.byte(last & used_bits(values.quantity()));
            }
        }
        Request::WriteMultipleRegisters { values, .. } => {
            writer.word(values.quantity());
            writer.byte(byte_count);
            writer.bytes(values.as_bytes());
        }
    }
    Ok(len)
}

/// Reads the response `pdu` to `request` and gives its data (V1.1b3 §6, §7). A response answers
/// the request when it carries the request's function code, and then: for a read, a byte count of
/// exactly the requested quantity and that many bytes; for a write, the request's address and its
/// value (codes 5 and 6) or quantity (codes 15 and 16). The bits past the quantity in the last byte
/// of a read of coils or inputs are not read.
///
/// # Errors
///
/// - [`Error::Exception`] for an exception response: the server received the request and did not
///   carry it out.
/// - [`Error::UnexpectedFunction`], [`Error::ByteCountMismatch`], [`Error::ResponseLength`],
///   [`Error::AddressMismatch`], [`Error::ValueMismatch`] and [`Error::QuantityMismatch`] for a
///   response that does not answer `request`, each naming what is wrong.
/// - The errors of [`Request::validate`] for a request that cannot have been sent.
pub fn decode_response<'a>(request: &Request<'_>, pdu: &'a [u8]) -> Result<Response<'a>, Error> {
    let byte_count = request.byte_count()?;
    let function = request.function();
    let Some(&code) = pdu.first() else {
        return Err(Error::ResponseLength {
            expected: response_len(request, byte_count),
            actual: 0,
        });
    };
    if code == (function.code() | EXCEPTION_FLAG) {
        return Err(exception(function, pdu));
    }
    if code != function.code() {
        return Err(Error::UnexpectedFunction {
            expected: function,
            actual: code,
        });
    }
    match *request {
        Request::ReadCoils { quantity, .. } | Request::ReadDiscreteInputs { quantity, .. } => {
            let packed = read_data(pdu, byte_count)?;
            Ok(Response::Bits(Bits { quantity, packed }))
        }
        Request::ReadHoldingRegisters { quantity, .. }
        | Request::ReadInputRegisters { quantity, .. } => {
            let bytes = read_data(pdu, byte_count)?;
            Ok(Response::Registers(Registers { quantity, bytes }))
        }
        Request::WriteSingleCoil { address, value } => {
            written(pdu, address, Echo::Value(coil_word(value)))
        }
        Request::WriteSingleRegister { address, value } => {
            written(pdu, address, Echo::Value(value))
        }
        Request::WriteMultipleCoils { address, values } => {
            written(pdu, address, Echo::Quantity(values.quantity()))
        }
        Request::WriteMultipleRegisters { address, values } => {
            written(pdu, address, Echo::Quantity(values.quantity()))
        }
    }
}

/// The length of `request`'s PDU, after checking the request as [`Request::validate`] does.
pub(crate) fn request_len(request: &Request<'_>) -> Result<u16, Error> {
    let byte_count = request.byte_count()?;
    Ok(encoded_len(request, byte_count))
}

/// The length of the PDU of a checked request whose data has `byte_count` bytes.
fn encoded_len(request: &Request<'_>, byte_count: u8) -> u16 {
    match request {
        Request::ReadCoils { .. }
        | Request::ReadDiscreteInputs { .. }
        | Request::ReadHoldingRegisters { .. }
        | Request::ReadInputRegisters { .. }
        | Request::WriteSingleCoil { .. }
        | Request::WriteSingleRegister { .. } => TWO_WORDS_LEN,
        Request::WriteMultipleCoils { .. } | Request::WriteMultipleRegisters { .. } => {
            MULTIPLE_HEADER_LEN + u16::from(byte_count)
        }
    }
}

/// The length of the response that answers a checked request whose data has `byte_count` bytes:
/// the function code, the byte count and the data of a read, or the function code and two words of
/// a write.
fn response_len(request: &Request<'_>, byte_count: u8) -> usize {
    match request {
        Request::ReadCoils { .. }
        | Request::ReadDiscreteInputs { .. }
        | Request::ReadHoldingRegisters { .. }
        | Request::ReadInputRegisters { .. } => READ_HEADER_LEN + usize::from(byte_count),
        Request::WriteSingleCoil { .. }
        | Request::WriteSingleRegister { .. }
        | Request::WriteMultipleCoils { .. }
        | Request::WriteMultipleRegisters { .. } => usize::from(TWO_WORDS_LEN),
    }
}

/// The value of a Write Single Coil request: 0xFF00 for ON and 0x0000 for OFF (V1.1b3 §6.5).
const fn coil_word(on: bool) -> u16 {
    if on {
        COIL_ON
    } else {
        COIL_OFF
    }
}

/// The bits of the last byte of `quantity` packed coils that belong to a coil: all eight when the
/// quantity is a multiple of 8, else the low `quantity % 8`.
fn used_bits(quantity: u16) -> u8 {
    match quantity % 8 {
        0 => 0xFF,
        used => 0xFF_u8 >> (8 - used),
    }
}

/// The error of an exception response to `function`: [`Error::Exception`] with its code, or
/// [`Error::ResponseLength`] when the response is not the two bytes of V1.1b3 §7.
fn exception(function: FunctionCode, pdu: &[u8]) -> Error {
    match *pdu {
        [_, code] => Error::Exception {
            function,
            code: ExceptionCode::from_code(code),
        },
        _ => Error::ResponseLength {
            expected: EXCEPTION_LEN,
            actual: pdu.len(),
        },
    }
}

/// The data of a read response whose function code is checked: the bytes after the byte count,
/// when the byte count is `byte_count` and exactly that many bytes follow it (V1.1b3 §6.1 to
/// §6.4).
fn read_data(pdu: &[u8], byte_count: u8) -> Result<&[u8], Error> {
    let expected = READ_HEADER_LEN + usize::from(byte_count);
    let Some(&received) = pdu.get(1) else {
        return Err(Error::ResponseLength {
            expected,
            actual: pdu.len(),
        });
    };
    if received != byte_count {
        return Err(Error::ByteCountMismatch {
            expected: byte_count,
            actual: received,
        });
    }
    if pdu.len() != expected {
        return Err(Error::ResponseLength {
            expected,
            actual: pdu.len(),
        });
    }
    Ok(&pdu[READ_HEADER_LEN..])
}

/// What the second word of a write response echoes.
#[derive(Clone, Copy)]
enum Echo {
    /// The value of Write Single Coil or Write Single Register (V1.1b3 §6.5, §6.6).
    Value(u16),
    /// The quantity of Write Multiple Coils or Write Multiple Registers (V1.1b3 §6.11, §6.12).
    Quantity(u16),
}

/// Checks a write response whose function code is checked: five bytes, the request's `address`,
/// and the `echo` of its value or quantity.
fn written(pdu: &[u8], address: u16, echo: Echo) -> Result<Response<'_>, Error> {
    let [_, address_high, address_low, echo_high, echo_low] = *pdu else {
        return Err(Error::ResponseLength {
            expected: usize::from(TWO_WORDS_LEN),
            actual: pdu.len(),
        });
    };
    let echoed_address = u16::from_be_bytes([address_high, address_low]);
    if echoed_address != address {
        return Err(Error::AddressMismatch {
            expected: address,
            actual: echoed_address,
        });
    }
    let echoed = u16::from_be_bytes([echo_high, echo_low]);
    match echo {
        Echo::Value(expected) if echoed != expected => Err(Error::ValueMismatch {
            expected,
            actual: echoed,
        }),
        Echo::Quantity(expected) if echoed != expected => Err(Error::QuantityMismatch {
            expected,
            actual: echoed,
        }),
        Echo::Value(_) | Echo::Quantity(_) => Ok(Response::Written),
    }
}

/// Writes the fields of a PDU one after another into a buffer of exactly the PDU's length.
struct Writer<'a> {
    out: &'a mut [u8],
    len: usize,
}

impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        let end = self.len + bytes.len();
        self.out[self.len..end].copy_from_slice(bytes);
        self.len = end;
    }

    fn byte(&mut self, byte: u8) {
        self.bytes(&[byte]);
    }

    fn word(&mut self, word: u16) {
        self.bytes(&word.to_be_bytes());
    }
}
