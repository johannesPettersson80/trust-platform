//! Modbus RTU on a serial line (Modbus over Serial Line Specification and Implementation Guide
//! V1.02 §2.1, §2.2, §2.5.1; Modbus Application Protocol Specification V1.1b3 §4.1): a frame is
//! the server's address, the PDU and the CRC-16, at most 256 bytes, and silent intervals separate
//! the frames on the line.
//!
//! Servers have the addresses 1 to 247; 248 to 255 are reserved. Address 0 is the broadcast:
//! every server carries out the request and none answers it, so a broadcast is a write and the
//! driver waits for no response ([`expects_response`]).

use crate::pdu::{self, Request, Response};
use crate::Error;

/// The greatest RTU frame: the address, a PDU of 253 bytes and the CRC (V1.02 §2.5.1).
pub const MAX_ADU_LEN: usize = 256;
/// The broadcast address (V1.02 §2.2).
pub const BROADCAST_ADDRESS: u8 = 0;
/// The greatest server address (V1.02 §2.2); 248 to 255 are reserved.
pub const MAX_SERVER_ADDRESS: u8 = 247;

/// The least frame: the address, a function code and the CRC (V1.02 §2.5.1, figure 12: "Data 0 up
/// to 252 byte(s)").
const MIN_ADU_LEN: usize = 4;
/// The bytes of the CRC.
const CRC_LEN: usize = 2;
/// The value the CRC register starts at (V1.02 §6.2.2).
const CRC_PRESET: u16 = 0xFFFF;
/// The polynomial the CRC register is combined with, 1 + x^2 + x^15 + x^16 with its bits in
/// reverse order (V1.02 §6.2.2).
const CRC_POLYNOMIAL: u16 = 0xA001;
/// The bits of one RTU character: a start bit, eight data bits, a parity bit (or a second stop
/// bit when there is no parity) and a stop bit (V1.02 §2.5.1).
const BITS_PER_CHARACTER: u32 = 11;
/// The highest baud rate at which the silent intervals are character times; above it they are
/// fixed (V1.02 §2.5.1.1).
const CHARACTER_TIMED_MAX_BAUD: u32 = 19_200;
/// The inter-frame delay above 19,200 baud: 1.750 ms (V1.02 §2.5.1.1).
const FIXED_INTER_FRAME_DELAY_US: u32 = 1750;
/// The inter-character time-out above 19,200 baud: 750 µs (V1.02 §2.5.1.1).
const FIXED_INTER_CHARACTER_TIMEOUT_US: u32 = 750;
/// 3.5 characters times the microseconds of a second: 3.5 × 11 bits × 1,000,000 = 38,500,000.
/// Divided by the baud rate it gives the inter-frame delay in microseconds.
const INTER_FRAME_BIT_MICROSECONDS: u32 = 7 * BITS_PER_CHARACTER * 1_000_000 / 2;
/// 1.5 characters times the microseconds of a second: 1.5 × 11 bits × 1,000,000 = 16,500,000.
/// Divided by the baud rate it gives the inter-character time-out in microseconds.
const INTER_CHARACTER_BIT_MICROSECONDS: u32 = 3 * BITS_PER_CHARACTER * 1_000_000 / 2;

/// The CRC-16 of the bytes of an RTU frame before its CRC (V1.02 §2.5.1.2, §6.2.2). The register
/// starts at 0xFFFF. Each byte is combined into its low byte by exclusive OR; then, eight times,
/// the register shifts one bit toward its least significant bit, and when the bit shifted out is
/// 1 the register is combined with 0xA001 by exclusive OR. A frame carries the result low byte
/// first.
#[must_use]
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = CRC_PRESET;
    for &byte in bytes {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            let shifted_out = (crc & 1) == 1;
            crc >>= 1;
            if shifted_out {
                crc ^= CRC_POLYNOMIAL;
            }
        }
    }
    crc
}

/// Whether a server answers a request to `address`: every server address does, and the broadcast
/// address 0 does not (V1.02 §2.1).
#[must_use]
pub const fn expects_response(address: u8) -> bool {
    address != BROADCAST_ADDRESS
}

/// Writes `request` to the server at `address` as an RTU frame at the start of `out`, and gives
/// the frame's length: the address, the PDU that [`pdu::encode_request`] writes, and the
/// [`crc16`] of both, low byte first (V1.02 §2.5.1.2). Address 0 broadcasts the request, which
/// only a write may be.
///
/// # Errors
///
/// [`Error::ReservedAddress`] for an address from 248 to 255; [`Error::BroadcastRead`] for a read
/// to address 0; the errors of [`Request::validate`]; and [`Error::BufferTooSmall`] when `out` is
/// shorter than the frame. A refused request writes nothing.
pub fn encode_request(address: u8, request: &Request<'_>, out: &mut [u8]) -> Result<usize, Error> {
    if address > MAX_SERVER_ADDRESS {
        return Err(Error::ReservedAddress { address });
    }
    let function = request.function();
    if address == BROADCAST_ADDRESS && !function.is_write() {
        return Err(Error::BroadcastRead { function });
    }
    let body_len = 1 + usize::from(pdu::request_len(request)?);
    let len = body_len + CRC_LEN;
    let available = out.len();
    let frame = out.get_mut(..len).ok_or(Error::BufferTooSmall {
        needed: len,
        available,
    })?;
    let (body, check) = frame.split_at_mut(body_len);
    let (address_byte, pdu_bytes) = body.split_at_mut(1);
    pdu::encode_request(request, pdu_bytes)?;
    address_byte[0] = address;
    check.copy_from_slice(&crc16(body).to_le_bytes());
    Ok(len)
}

/// Reads the RTU frame `adu` that answers `request`, which went to the server at `address`, and
/// gives the response's data as [`pdu::decode_response`] does. The CRC is checked first and the
/// address after it, as a receiver does (V1.02 §2.5.1.1).
///
/// # Errors
///
/// - [`Error::NoResponseToBroadcast`] for address 0 and [`Error::ReservedAddress`] for 248 to
///   255, which no server answers from.
/// - [`Error::FrameLength`] for a frame shorter than 4 or longer than 256 bytes.
/// - [`Error::CrcMismatch`] when the CRC does not match the frame's bytes.
/// - [`Error::UnitMismatch`] for a frame from another server.
/// - The errors of [`pdu::decode_response`].
pub fn decode_response<'a>(
    address: u8,
    request: &Request<'_>,
    adu: &'a [u8],
) -> Result<Response<'a>, Error> {
    if address == BROADCAST_ADDRESS {
        return Err(Error::NoResponseToBroadcast);
    }
    if address > MAX_SERVER_ADDRESS {
        return Err(Error::ReservedAddress { address });
    }
    let frame_length = Error::FrameLength {
        len: adu.len(),
        min: MIN_ADU_LEN,
        max: MAX_ADU_LEN,
    };
    if !(MIN_ADU_LEN..=MAX_ADU_LEN).contains(&adu.len()) {
        return Err(frame_length);
    }
    let (body, check) = adu.split_last_chunk::<CRC_LEN>().ok_or(frame_length)?;
    let received = u16::from_le_bytes(*check);
    let computed = crc16(body);
    if received != computed {
        return Err(Error::CrcMismatch { computed, received });
    }
    let (&server, pdu_bytes) = body.split_first().ok_or(frame_length)?;
    if server != address {
        return Err(Error::UnitMismatch {
            expected: address,
            actual: server,
        });
    }
    pdu::decode_response(request, pdu_bytes)
}

/// The silent intervals of an RTU line in microseconds, each rounded up so that no silence is
/// shorter than the specification asks (V1.02 §2.5.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentIntervals {
    /// t3.5, the inter-frame delay: the least silence between two frames. A receiver takes a
    /// silence this long as the end of a frame.
    pub inter_frame_delay_us: u32,
    /// t1.5, the inter-character time-out: a longer silence between two characters of one frame
    /// leaves the frame incomplete, and the receiver discards it.
    pub inter_character_timeout_us: u32,
}

/// The silent intervals at `baud` bits per second (V1.02 §2.5.1.1): up to 19,200 baud, 3.5 and
/// 1.5 times the 11 bits of an RTU character; above 19,200 baud, the fixed 1,750 µs and 750 µs.
///
/// # Errors
///
/// [`Error::ZeroBaudRate`] for 0 baud, which has no character time.
pub fn silent_intervals(baud: u32) -> Result<SilentIntervals, Error> {
    if baud == 0 {
        return Err(Error::ZeroBaudRate);
    }
    if baud > CHARACTER_TIMED_MAX_BAUD {
        return Ok(SilentIntervals {
            inter_frame_delay_us: FIXED_INTER_FRAME_DELAY_US,
            inter_character_timeout_us: FIXED_INTER_CHARACTER_TIMEOUT_US,
        });
    }
    Ok(SilentIntervals {
        inter_frame_delay_us: INTER_FRAME_BIT_MICROSECONDS.div_ceil(baud),
        inter_character_timeout_us: INTER_CHARACTER_BIT_MICROSECONDS.div_ceil(baud),
    })
}
