//! A cycle trace that a runtime keeps, read over the control link (specification 11 §6.8.1, "Cycle
//! traces"; specification 34 PORT-05): its state, then its file part by part, each part's CRC-32
//! and the whole file's length and SHA-256 checked against what its state gave. `trust-runtime ctl
//! record`, `ctl fault-trace` and `trust_sim`'s machine tests share it.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{ControlClient, ControlError};

/// A trace that has ended, as `trace.read` gives its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndedTrace {
    /// Why it ended (specification 48 Appendix C, C.3), such as `complete` or `faulted`.
    pub reason: String,
    /// The cycles it holds.
    pub cycles: u64,
    /// The bytes of its file.
    pub bytes: usize,
    /// Its file's SHA-256, in hexadecimal.
    pub sha256: String,
}

/// A trace's state as `trace.read` answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceState {
    /// No trace: the flight recorder has kept no fault's trace.
    None,
    /// A recording that runs, with the cycles it holds so far.
    Recording {
        /// The cycles recorded so far.
        cycles: u64,
    },
    /// A trace that has ended.
    Ended(EndedTrace),
}

impl TraceState {
    /// The state that `state`, an answer of `trace.read` or `trace.stop`, gives.
    ///
    /// # Errors
    ///
    /// [`ControlError::Malformed`] for an answer that names no state, and for an ended trace without
    /// its reason, cycles, bytes or SHA-256, against which its file could not be checked.
    pub fn from_json(state: &Value) -> Result<Self, ControlError> {
        let text = |key: &str| state.get(key).and_then(Value::as_str).map(str::to_owned);
        let number = |key: &str| state.get(key).and_then(Value::as_u64);
        match state.get("state").and_then(Value::as_str) {
            Some("none") => Ok(Self::None),
            Some("recording") => Ok(Self::Recording {
                cycles: number("cycles").unwrap_or(0),
            }),
            Some("ended") => {
                let bytes = number("bytes").and_then(|bytes| usize::try_from(bytes).ok());
                match (text("reason"), number("cycles"), bytes, text("sha256")) {
                    (Some(reason), Some(cycles), Some(bytes), Some(sha256)) => {
                        Ok(Self::Ended(EndedTrace {
                            reason,
                            cycles,
                            bytes,
                            sha256,
                        }))
                    }
                    _ => Err(ControlError::Malformed(format!(
                        "an ended trace without its reason, cycles, bytes or SHA-256: {state}"
                    ))),
                }
            }
            _ => Err(ControlError::Malformed(format!(
                "no trace's state: {state}"
            ))),
        }
    }
}

/// The flight recorder's trace of the last fault of the runtime that `client` is connected to: its
/// state and its file, read and checked as [`read_file`] reads it; `None` before a fault, and for a
/// runtime without a flight recorder.
///
/// # Errors
///
/// The connection failed, the runtime refused `trace.read`, or an answer is not what the request
/// answers: a fault's trace that still records, a part that is not whole, or a file of another
/// length or SHA-256 than its state gave.
pub fn fault_trace(
    client: &mut ControlClient,
) -> Result<Option<(EndedTrace, Vec<u8>)>, ControlError> {
    let state = client.request("trace.read", Some(json!({ "trace": "fault" })))?;
    match TraceState::from_json(&state)? {
        TraceState::None => Ok(None),
        TraceState::Recording { .. } => Err(ControlError::Malformed(
            "a fault's trace that is still recording".into(),
        )),
        TraceState::Ended(ended) => {
            let file = read_file(client, &json!("fault"), &ended)?;
            Ok(Some((ended, file)))
        }
    }
}

/// The ended trace `trace`'s file (its id, or `"fault"`), read part by part from offset 0, each
/// part checked ([`part`]), and the whole checked against the length and the SHA-256 that `ended`
/// gives.
///
/// # Errors
///
/// The connection failed, the runtime refused a part, a part is not whole, or the file has
/// another length or SHA-256 than `ended`.
pub fn read_file(
    client: &mut ControlClient,
    trace: &Value,
    ended: &EndedTrace,
) -> Result<Vec<u8>, ControlError> {
    let mut file = Vec::with_capacity(ended.bytes);
    while file.len() < ended.bytes {
        let reply = client.request(
            "trace.read",
            Some(json!({ "trace": trace, "offset": file.len() })),
        )?;
        let bytes = part(&reply, file.len())?;
        file.extend_from_slice(&bytes);
    }
    let sha256 = hex(&Sha256::digest(&file));
    if file.len() != ended.bytes || sha256 != ended.sha256 {
        return Err(ControlError::Malformed(format!(
            "the trace read has {} bytes and the SHA-256 {sha256}, not the {} bytes and the \
             SHA-256 {} that it ended with",
            file.len(),
            ended.bytes,
            ended.sha256
        )));
    }
    Ok(file)
}

/// The bytes of the part that `reply` carries, asked for at `offset`: its offset, its next offset
/// and its CRC-32 agree with its bytes, which are never none (specification 34 PORT-05).
///
/// # Errors
///
/// [`ControlError::Malformed`] for a part without its offset, next offset, data or CRC-32, from
/// another offset, empty, with a next offset its bytes do not reach, or whose CRC-32 differs.
pub fn part(reply: &Value, offset: usize) -> Result<Vec<u8>, ControlError> {
    let number = |key: &str| {
        reply
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
    };
    let data = reply
        .get("data")
        .and_then(Value::as_str)
        .and_then(|data| STANDARD.decode(data).ok());
    let crc32 = reply.get("crc32").and_then(Value::as_str);
    let (Some(at), Some(next), Some(bytes), Some(crc32)) =
        (number("offset"), number("next_offset"), data, crc32)
    else {
        return Err(ControlError::Malformed(format!(
            "a part without its offset, next offset, data or CRC-32: {reply}"
        )));
    };
    if at != offset || bytes.is_empty() || next != offset + bytes.len() {
        return Err(ControlError::Malformed(format!(
            "the part asked for at {offset} came from {at} with {} bytes and the next offset \
             {next}",
            bytes.len()
        )));
    }
    let checksum = format!("{:08x}", crc32fast::hash(&bytes));
    if checksum != crc32 {
        return Err(ControlError::Malformed(format!(
            "the part at {offset} has the CRC-32 {checksum}, not the {crc32} it came with"
        )));
    }
    Ok(bytes)
}

/// `bytes` in lower-case hexadecimal.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing to a `String` cannot fail.
        let _ = write!(text, "{byte:02x}");
    }
    text
}
