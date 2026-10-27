//! Modbus TCP: the MBAP header before each PDU (Modbus Messaging on TCP/IP Implementation Guide
//! V1.0b §3.1.2, §3.1.3; Modbus Application Protocol Specification V1.1b3 §4.1).
//!
//! The header is seven bytes, each field high byte first: the transaction identifier, which the
//! server copies into its response; the protocol identifier, 0 for Modbus; the length, which counts
//! the bytes after it, the unit identifier and the PDU; and the unit identifier, which the server
//! copies too. A frame is at most 260 bytes: the header and a PDU of at most 253.
//!
//! A driver that reads a TCP stream reads the seven header bytes first and gives them to
//! [`MbapHeader::decode`], which refuses a header that is not Modbus before more is read. It then
//! reads the [`MbapHeader::pdu_len`] bytes that follow, and hands the whole frame to
//! [`decode_response`].

use crate::pdu::{self, Request, Response};
use crate::Error;

/// The bytes of the MBAP header.
pub const HEADER_LEN: usize = 7;
/// The greatest Modbus TCP frame: the header and a PDU of 253 bytes (V1.1b3 §4.1).
pub const MAX_ADU_LEN: usize = HEADER_LEN + pdu::MAX_PDU_LEN;

/// The least response frame: the header and a function code.
const MIN_ADU_LEN: usize = HEADER_LEN + 1;
/// The protocol identifier of Modbus (V1.0b §3.1.3).
const PROTOCOL_ID: u16 = 0;
/// The least length: the unit identifier and a function code.
const MIN_LENGTH: u16 = 2;
/// The greatest length: the unit identifier and a PDU of 253 bytes.
const MAX_LENGTH: u16 = 254;
/// The bytes of the header that its length does not count: the transaction identifier, the
/// protocol identifier and the length itself.
const UNCOUNTED_LEN: usize = 6;

/// The MBAP header of a received frame (V1.0b §3.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbapHeader {
    /// The transaction identifier, which pairs a response with its request.
    pub transaction_id: u16,
    /// The count of the bytes after the length field: the unit identifier and the PDU.
    pub length: u16,
    /// The unit identifier: the server behind a gateway, or 0xFF for a server on TCP/IP itself
    /// (V1.0b §4.4.1.2).
    pub unit_id: u8,
}

impl MbapHeader {
    /// Reads the seven header bytes of a received frame.
    ///
    /// # Errors
    ///
    /// [`Error::ProtocolId`] when the protocol identifier is not 0, and [`Error::MbapLength`]
    /// when the length is outside 2 to 254, which would make the frame shorter than a function
    /// code or longer than 260 bytes.
    pub fn decode(bytes: [u8; HEADER_LEN]) -> Result<Self, Error> {
        let protocol_id = u16::from_be_bytes([bytes[2], bytes[3]]);
        if protocol_id != PROTOCOL_ID {
            return Err(Error::ProtocolId {
                actual: protocol_id,
            });
        }
        let length = u16::from_be_bytes([bytes[4], bytes[5]]);
        if !(MIN_LENGTH..=MAX_LENGTH).contains(&length) {
            return Err(Error::MbapLength { length });
        }
        Ok(Self {
            transaction_id: u16::from_be_bytes([bytes[0], bytes[1]]),
            length,
            unit_id: bytes[6],
        })
    }

    /// How many bytes follow the header: the PDU, the length less the unit identifier.
    #[must_use]
    pub fn pdu_len(self) -> usize {
        usize::from(self.length).saturating_sub(1)
    }
}

/// Writes `request` as a Modbus TCP frame at the start of `out` and gives the frame's length: the
/// MBAP header with `transaction_id`, protocol identifier 0, the length of the unit identifier and
/// the PDU, and `unit_id`; then the PDU that [`pdu::encode_request`] writes.
///
/// # Errors
///
/// The errors of [`Request::validate`], and [`Error::BufferTooSmall`] when `out` is shorter than
/// the frame. A refused request writes nothing.
pub fn encode_request(
    transaction_id: u16,
    unit_id: u8,
    request: &Request<'_>,
    out: &mut [u8],
) -> Result<usize, Error> {
    let pdu_len = pdu::request_len(request)?;
    let len = HEADER_LEN + usize::from(pdu_len);
    let available = out.len();
    let frame = out.get_mut(..len).ok_or(Error::BufferTooSmall {
        needed: len,
        available,
    })?;
    let (header, body) = frame.split_at_mut(HEADER_LEN);
    pdu::encode_request(request, body)?;
    let [transaction_high, transaction_low] = transaction_id.to_be_bytes();
    let [protocol_high, protocol_low] = PROTOCOL_ID.to_be_bytes();
    let [length_high, length_low] = (pdu_len + 1).to_be_bytes();
    header.copy_from_slice(&[
        transaction_high,
        transaction_low,
        protocol_high,
        protocol_low,
        length_high,
        length_low,
        unit_id,
    ]);
    Ok(len)
}

/// Reads the Modbus TCP frame `adu` that answers `request`, which went out with `transaction_id`,
/// and gives the response's data as [`pdu::decode_response`] does. The frame must be Modbus, its
/// length must count the bytes that follow it, and its transaction identifier must be the request's
/// (V1.0b §3.1.3, §4.4.1.3). Its unit identifier is not compared: V1.0b §4.4.1.3 lets a client
/// ignore it for a server directly on TCP/IP, and truST does (specification 11 §6.5).
///
/// # Errors
///
/// - [`Error::FrameLength`] for a frame shorter than 8 or longer than 260 bytes.
/// - The errors of [`MbapHeader::decode`], and [`Error::MbapLengthMismatch`] when the length does
///   not count the bytes that follow it.
/// - [`Error::TransactionMismatch`] for a response to another transaction, which the client
///   discards and reads on.
/// - The errors of [`pdu::decode_response`].
pub fn decode_response<'a>(
    transaction_id: u16,
    request: &Request<'_>,
    adu: &'a [u8],
) -> Result<Response<'a>, Error> {
    let frame_length = Error::FrameLength {
        len: adu.len(),
        min: MIN_ADU_LEN,
        max: MAX_ADU_LEN,
    };
    if !(MIN_ADU_LEN..=MAX_ADU_LEN).contains(&adu.len()) {
        return Err(frame_length);
    }
    let (header, body) = adu.split_first_chunk::<HEADER_LEN>().ok_or(frame_length)?;
    let header = MbapHeader::decode(*header)?;
    let following = adu.len() - UNCOUNTED_LEN;
    if usize::from(header.length) != following {
        return Err(Error::MbapLengthMismatch {
            length: header.length,
            following,
        });
    }
    if header.transaction_id != transaction_id {
        return Err(Error::TransactionMismatch {
            expected: transaction_id,
            actual: header.transaction_id,
        });
    }
    pdu::decode_response(request, body)
}
