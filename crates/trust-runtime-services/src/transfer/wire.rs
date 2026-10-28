//! The JSON shapes of the transfer operations (specification 34, PORT-05). The parameters of
//! `transfer.begin`, `transfer.part`, `transfer.status`, `transfer.abort` and `transfer.get` are
//! read here, and their results and the head of a reply above the frame are built here. The data
//! of a part and of a reply is standard base64 (RFC 4648), and every digest and CRC-32 is written
//! as hexadecimal. A part's base64 is decoded by the session into its part buffer, and the rules
//! themselves are in [`crate::transfer::TransferSession`].

use alloc::string::{String, ToString};
use core::fmt::{self, Write as _};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ids::TransferId;
use super::session::{ReplyHead, ReplyPart, State, Status, PART_BYTES};

/// A request the transfer operations cannot read. It is answered `invalid request: <reason>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// The parameters are not the object the operation names; the reason is the JSON error.
    Params(String),
    /// A member is not in the shape the operation gives it.
    Member {
        /// The member's name.
        name: &'static str,
        /// The shape the member must have.
        expected: &'static str,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Params(reason) => write!(formatter, "invalid request: {reason}"),
            Self::Member { name, expected } => {
                write!(formatter, "invalid request: {name} must be {expected}")
            }
        }
    }
}

impl core::error::Error for WireError {}

/// The parameters of `transfer.begin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeginRequest {
    /// The size of the whole container, in bytes.
    pub bytes: u64,
    /// The SHA-256 of the whole container.
    pub digest: [u8; 32],
    /// Whether the upload is a chip's firmware (`"for": "firmware"`, specification 34 TARGET-04)
    /// rather than a program (`"for": "program"`, the default).
    pub firmware: bool,
}

/// The parameters of `transfer.part`: one part, its base64 and its CRC-32. The base64 is borrowed
/// from the request; the session decodes it into its part buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartRequest<'a> {
    /// The upload the part belongs to.
    pub transfer: TransferId,
    /// The offset of the part's first byte.
    pub offset: u64,
    /// The part's bytes as standard base64.
    pub encoded: &'a str,
    /// The CRC-32 of the decoded bytes.
    pub crc32: u32,
}

/// The parameters of `transfer.get`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRequest {
    /// The held reply to read.
    pub transfer: TransferId,
    /// The offset to read from.
    pub offset: u64,
    /// The number of bytes to read, at most a part.
    pub len: u64,
}

#[derive(Deserialize)]
struct BeginParams<'a> {
    bytes: u64,
    sha256: &'a str,
    #[serde(rename = "for", default)]
    target: Option<&'a str>,
}

#[derive(Deserialize)]
struct PartParams<'a> {
    transfer: &'a str,
    offset: u64,
    data: &'a str,
    crc32: &'a str,
}

#[derive(Deserialize)]
struct GetParams<'a> {
    transfer: &'a str,
    offset: u64,
    len: u64,
}

#[derive(Deserialize)]
struct TransferParams<'a> {
    transfer: &'a str,
}

/// Reads the parameters of `transfer.begin`.
///
/// # Errors
///
/// [`WireError::Params`] when `params` is not an object with `bytes` and `sha256`, and
/// [`WireError::Member`] when `sha256` is not 64 hexadecimal digits or `for` is neither `program`
/// nor `firmware`.
pub fn begin_request(params: &Value) -> Result<BeginRequest, WireError> {
    let params =
        BeginParams::deserialize(params).map_err(|err| WireError::Params(err.to_string()))?;
    let digest = hex_array::<32>(params.sha256).ok_or(WireError::Member {
        name: "sha256",
        expected: "64 hexadecimal digits",
    })?;
    let firmware = match params.target {
        None | Some("program") => false,
        Some("firmware") => true,
        Some(_) => {
            return Err(WireError::Member {
                name: "for",
                expected: "\"program\" or \"firmware\"",
            })
        }
    };
    Ok(BeginRequest {
        bytes: params.bytes,
        digest,
        firmware,
    })
}

/// Reads the parameters of `transfer.part`. The base64 in `data` is not decoded here: the session
/// decodes it into its part buffer and refuses text that is not canonical base64.
///
/// # Errors
///
/// [`WireError::Params`] when `params` lacks a member or has one of the wrong JSON type, and
/// [`WireError::Member`] when `transfer` is not 16 hexadecimal digits or `crc32` is not 8
/// hexadecimal digits.
pub fn part_request(params: &Value) -> Result<PartRequest<'_>, WireError> {
    let params =
        PartParams::deserialize(params).map_err(|err| WireError::Params(err.to_string()))?;
    let crc32 = hex_array::<4>(params.crc32).ok_or(WireError::Member {
        name: "crc32",
        expected: "8 hexadecimal digits",
    })?;
    Ok(PartRequest {
        transfer: transfer_id(params.transfer)?,
        offset: params.offset,
        encoded: params.data,
        crc32: u32::from_be_bytes(crc32),
    })
}

/// Reads the parameters of `transfer.get`.
///
/// # Errors
///
/// [`WireError::Params`] when `params` lacks a member or has one of the wrong JSON type, and
/// [`WireError::Member`] when `transfer` is not 16 hexadecimal digits.
pub fn get_request(params: &Value) -> Result<GetRequest, WireError> {
    let params =
        GetParams::deserialize(params).map_err(|err| WireError::Params(err.to_string()))?;
    Ok(GetRequest {
        transfer: transfer_id(params.transfer)?,
        offset: params.offset,
        len: params.len,
    })
}

/// Reads the `transfer` member of the parameters of `transfer.status` and `transfer.abort`.
///
/// # Errors
///
/// [`WireError::Params`] when `params` lacks `transfer`, and [`WireError::Member`] when it is not
/// 16 hexadecimal digits.
pub fn transfer_request(params: &Value) -> Result<TransferId, WireError> {
    let params =
        TransferParams::deserialize(params).map_err(|err| WireError::Params(err.to_string()))?;
    transfer_id(params.transfer)
}

// The reply builders below allocate the reply's text, as every control reply does. The transfer's
// own buffers are reserved only at begin, so none of them is allocated here.

/// The `result` of `transfer.begin`: the id, the part size and the next offset, which is 0.
#[must_use]
pub fn begin_reply(transfer: TransferId) -> Value {
    json!({"transfer": transfer.to_string(), "part_bytes": PART_BYTES, "next_offset": 0})
}

/// The `result` of `transfer.part`: the next offset.
#[must_use]
pub fn part_reply(next_offset: usize) -> Value {
    json!({"next_offset": next_offset})
}

/// The `result` of `transfer.status`.
#[must_use]
pub fn status_reply(status: Status) -> Value {
    let state = match status.state {
        State::Receiving => "receiving",
        State::Complete => "complete",
    };
    json!({"state": state, "next_offset": status.next_offset, "bytes": status.bytes})
}

/// The `result` of `transfer.abort`: nothing to report.
#[must_use]
pub fn abort_reply() -> Value {
    json!({})
}

/// The `result` of a request whose reply is held: the id to read it by, its length, its SHA-256
/// and the part size.
#[must_use]
pub fn reply_head(head: &ReplyHead) -> Value {
    json!({
        "reply_transfer": head.transfer.to_string(),
        "bytes": head.bytes,
        "sha256": hex_text(&head.sha256),
        "part_bytes": PART_BYTES,
    })
}

/// The `result` of `transfer.get`: the offset, the base64 of the part and its CRC-32.
#[must_use]
pub fn get_reply(part: &ReplyPart<'_>) -> Value {
    json!({
        "offset": part.offset,
        "data": STANDARD.encode(&part.data),
        "crc32": hex_text(&part.crc32.to_be_bytes()),
    })
}

fn transfer_id(text: &str) -> Result<TransferId, WireError> {
    let [b0, b1, b2, b3, s0, s1, s2, s3] = hex_array::<8>(text).ok_or(WireError::Member {
        name: "transfer",
        expected: "16 hexadecimal digits",
    })?;
    Ok(TransferId::new(
        u32::from_be_bytes([b0, b1, b2, b3]),
        u32::from_be_bytes([s0, s1, s2, s3]),
    ))
}

/// The `N` bytes that the `2 * N` hexadecimal digits of `text` spell, in either case, or `None`
/// for another length or a character that is not a hexadecimal digit.
fn hex_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    let (pairs, _) = digits.as_chunks::<2>();
    for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
        *byte = (hex_digit(*high)? << 4) | hex_digit(*low)?;
    }
    Some(bytes)
}

fn hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

/// The lowercase hexadecimal digits of `bytes`, two per byte.
pub(crate) fn hex_text(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing into a String cannot fail.
        let _ = write!(text, "{byte:02x}");
    }
    text
}
