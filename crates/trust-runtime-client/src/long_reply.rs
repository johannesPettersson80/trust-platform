//! A reply that the runtime holds whole because it is longer than the link's frame (specification
//! 34, PORT-05, "Reply head and get" and "The client: a reply longer than the frame").
//!
//! The runtime answers such a request with a reply head, `{reply_transfer, bytes, sha256,
//! part_bytes}`. The client reads the reply with `transfer.get` in parts of 4,096 bytes, checks
//! each part's CRC-32 and the whole reply's SHA-256, and hands the reply on as if it had come in one
//! line. When any of it fails it frees the runtime's reply buffer with `transfer.abort` and reports
//! the first failure; it never hands on part of a reply.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{result_of, ControlClient, ControlError};

/// The most bytes that one `transfer.get` reads (specification 34, PORT-05).
const PART_BYTES: usize = 4_096;

/// The head of a reply that the runtime holds.
#[derive(Debug)]
pub(crate) struct ReplyHead {
    /// The id that names the held reply: 16 hexadecimal digits.
    transfer: String,
    /// The length of the held reply, in bytes.
    bytes: usize,
    /// The SHA-256 of the held reply: 64 lowercase hexadecimal digits.
    sha256: String,
}

/// The reply head that `response` is, or `None` when it is no head: a head is a successful
/// response whose result is an object with a `reply_transfer` member.
///
/// # Errors
/// `response` is a head that is not well formed.
pub(crate) fn head_of(response: &Value) -> Result<Option<ReplyHead>, ControlError> {
    if response.get("ok") != Some(&Value::Bool(true)) {
        return Ok(None);
    }
    let Some(result) = response.get("result").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(transfer) = result.get("reply_transfer") else {
        return Ok(None);
    };
    let malformed =
        |what: &str| ControlError::Malformed(format!("the reply head is malformed: {what}"));
    let transfer = transfer
        .as_str()
        .filter(|text| is_hex(text, 16))
        .ok_or_else(|| malformed("reply_transfer is not 16 hexadecimal digits"))?;
    let bytes = result
        .get("bytes")
        .and_then(Value::as_u64)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| malformed("bytes is not a whole number"))?;
    let sha256 = result
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|text| is_hex(text, 64))
        .ok_or_else(|| malformed("sha256 is not 64 hexadecimal digits"))?;
    Ok(Some(ReplyHead {
        transfer: transfer.to_string(),
        bytes,
        sha256: sha256.to_ascii_lowercase(),
    }))
}

impl ControlClient {
    /// Reads the reply that `head` names and returns it as the response object. When that fails,
    /// it frees the runtime's reply buffer first, and returns the first failure.
    pub(crate) fn read_held(
        &mut self,
        kind: &str,
        head: &ReplyHead,
    ) -> Result<Value, ControlError> {
        let outcome = self.get_whole(kind, head);
        if outcome.is_err() {
            // The first failure is the one reported. An abort that fails too only leaves the
            // buffer to the next held reply or to a reset.
            let _ = self.round_trip("transfer.abort", Some(json!({ "transfer": head.transfer })));
        }
        outcome
    }

    /// Reads the held reply in parts and checks it against its head.
    fn get_whole(&mut self, kind: &str, head: &ReplyHead) -> Result<Value, ControlError> {
        // A reply of `bytes` bytes is a line of `bytes + 1` with its line end, and the limit on
        // a line counts the line end.
        if let Some(limit) = self.response_limit
            && head.bytes >= limit
        {
            return Err(ControlError::Malformed(format!(
                "the answer to {kind} is longer than {limit} bytes"
            )));
        }
        let mut whole = Vec::new();
        while whole.len() < head.bytes {
            let wanted = PART_BYTES.min(head.bytes - whole.len());
            whole.extend_from_slice(&self.get_part(head, whole.len(), wanted)?);
        }
        if sha256_hex(&whole) != head.sha256 {
            return Err(ControlError::Malformed(format!(
                "the answer to {kind} does not match the SHA-256 of its head"
            )));
        }
        let response: Value = serde_json::from_slice(&whole)
            .map_err(|error| ControlError::Malformed(error.to_string()))?;
        if response.is_object() {
            Ok(response)
        } else {
            Err(ControlError::Malformed(format!(
                "the answer to {kind} is not a JSON object"
            )))
        }
    }

    /// Reads the part of `wanted` bytes at `offset` and checks it.
    fn get_part(
        &mut self,
        head: &ReplyHead,
        offset: usize,
        wanted: usize,
    ) -> Result<Vec<u8>, ControlError> {
        let response = self.round_trip(
            "transfer.get",
            Some(json!({ "transfer": head.transfer, "offset": offset, "len": wanted })),
        )?;
        let part = result_of(response, "transfer.get")?;
        let malformed = |what: &str| {
            ControlError::Malformed(format!("the part of the reply at offset {offset} {what}"))
        };
        if part.get("offset").and_then(Value::as_u64) != u64::try_from(offset).ok() {
            return Err(malformed("is at another offset"));
        }
        let data = part
            .get("data")
            .and_then(Value::as_str)
            .and_then(|text| STANDARD.decode(text).ok())
            .ok_or_else(|| malformed("is not canonical base64"))?;
        if data.len() != wanted {
            return Err(malformed(&format!(
                "has {} bytes, not the {wanted} that were asked for",
                data.len()
            )));
        }
        let crc32 = part
            .get("crc32")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !crc32.eq_ignore_ascii_case(&format!("{:08x}", crc32fast::hash(&data))) {
            return Err(malformed("has the wrong crc32"));
        }
        Ok(data)
    }
}

/// Whether `text` is exactly `digits` hexadecimal digits.
fn is_hex(text: &str, digits: usize) -> bool {
    text.len() == digits && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The SHA-256 of `bytes` as 64 lowercase hexadecimal digits.
fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    digest
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            // Writing into a String cannot fail.
            let _ = write!(text, "{byte:02x}");
            text
        })
}
