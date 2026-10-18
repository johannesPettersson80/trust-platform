//! The route list of an ADS target, read by a search (specification 11, O20). The target's AMS
//! router lists its routes on the system service (AMS port 10000): index group 803 (0x323), one
//! entry per index offset from 0, until ADS error 0x716 ends the list. This is the read that the
//! `ads` crate's `adstool route list` makes. It is read-only and needs the route from the target
//! to this runtime; nothing here adds a route. The decoding is pure, and the reads come from the
//! transport in `host::ads`.

use encoding_rs::WINDOWS_1252;
use serde_json::{json, Value};

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::{AdsIndexRead, AdsIndexReadError};

use super::super::report::Deadline;

/// The index group of the route list (`ads::index::ROUTE_LIST`).
const ROUTE_LIST_INDEX_GROUP: u32 = 803;
/// The ADS error code that ends the route list; `adstool route list` stops at it.
const END_OF_ROUTE_LIST: u32 = 0x716;
/// The bytes of an entry's fixed header; the host text starts at byte 44.
const HEADER_BYTES: usize = 44;
/// The buffer that each entry is read into, as `adstool route list` reads it.
const ENTRY_BYTES: usize = 2_048;

/// One route of the target's route table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Route {
    name: String,
    ams_net_id: String,
    host: String,
    temporary: bool,
    unidirectional: bool,
    virtual_nat: bool,
}

/// An entry of the route list that could not be decoded, with its index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DamagedEntry {
    index: u32,
    reason: String,
}

/// What a search reports of one target's route list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RouteList {
    /// The list was read to its end: its routes, and the entries that were damaged.
    Listed {
        routes: Vec<Route>,
        damaged: Vec<DamagedEntry>,
    },
    /// The list could not be read: why, and the target's error when it gave one.
    Unavailable {
        reason: String,
        error: Option<String>,
    },
}

impl RouteList {
    /// The `route_list` member of the target's find (specification 11, O20).
    pub(super) fn to_json(&self) -> Value {
        match self {
            Self::Listed { routes, damaged } => json!({
                "status": "ok",
                "routes": routes.iter().map(Route::to_json).collect::<Vec<_>>(),
                "damaged": damaged
                    .iter()
                    .map(|entry| json!({ "index": entry.index, "reason": entry.reason }))
                    .collect::<Vec<_>>(),
            }),
            Self::Unavailable {
                reason,
                error: Some(error),
            } => json!({ "status": "unavailable", "reason": reason, "error": error }),
            Self::Unavailable {
                reason,
                error: None,
            } => json!({ "status": "unavailable", "reason": reason }),
        }
    }
}

impl Route {
    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "ams_net_id": self.ams_net_id,
            "host": self.host,
            "temporary": self.temporary,
            "unidirectional": self.unidirectional,
            "virtual_nat": self.virtual_nat,
        })
    }
}

/// Reads the route list of `target` through `open`, which connects to its system service. Nothing
/// is opened or read once the search deadline has passed (specification 11, O20).
pub(super) fn route_list_for_target(
    target: &TargetIdentity,
    deadline: Deadline,
    open: impl FnOnce(&TargetIdentity) -> Result<Box<dyn AdsIndexRead>, String>,
) -> RouteList {
    if deadline.passed() {
        return deadline_passed();
    }
    match open(target) {
        Ok(mut reads) => read_route_list(reads.as_mut(), deadline),
        Err(error) => unavailable(
            "The route list could not be opened on the target's system service.",
            Some(error),
        ),
    }
}

/// Reads the route list through `reads`, entry by entry, until ADS error 0x716 ends it. A refused
/// read, or a read that fails before the end, makes the whole list unavailable: a partial or an
/// empty list is never reported in its place.
fn read_route_list(reads: &mut dyn AdsIndexRead, deadline: Deadline) -> RouteList {
    let mut routes = Vec::new();
    let mut damaged = Vec::new();
    let mut buffer = vec![0_u8; ENTRY_BYTES];
    for index in 0..u32::MAX {
        if deadline.passed() {
            return deadline_passed();
        }
        match reads.read_index(ROUTE_LIST_INDEX_GROUP, index, &mut buffer) {
            Ok(read) => match buffer.get(..read) {
                Some(entry) => match decode_entry(entry) {
                    Ok(route) => routes.push(route),
                    Err(reason) => damaged.push(DamagedEntry { index, reason }),
                },
                None => damaged.push(DamagedEntry {
                    index,
                    reason: format!(
                        "the read reports {read} bytes, more than the {ENTRY_BYTES}-byte buffer"
                    ),
                }),
            },
            Err(error) => match &error {
                AdsIndexReadError::Ads {
                    code: END_OF_ROUTE_LIST,
                    ..
                } => return RouteList::Listed { routes, damaged },
                AdsIndexReadError::Ads { .. } => {
                    return unavailable(
                        "The target refused the route read.",
                        Some(error.to_string()),
                    );
                }
                AdsIndexReadError::Transport(_) => {
                    let reason = if index == 0 {
                        "The target gave no answer to the route read, so there is no route from the target to this runtime. The search adds no route."
                    } else {
                        "The target stopped answering the route read before the list ended."
                    };
                    return unavailable(reason, Some(error.to_string()));
                }
            },
        }
    }
    unavailable("The route list has no end marker.", None)
}

/// Decodes one entry (specification 11, O20): a 44-byte header, then the host text and the name
/// text. Every length is checked against the bytes read, so a damaged entry is reported, never a
/// panic.
fn decode_entry(bytes: &[u8]) -> Result<Route, String> {
    let Some(header) = bytes.first_chunk::<HEADER_BYTES>() else {
        return Err(format!(
            "the entry is {} bytes, shorter than its {HEADER_BYTES}-byte header",
            bytes.len()
        ));
    };
    let host_len = header_len(header, 32);
    let name_len = header_len(header, 36);
    if host_len == 0 || name_len == 0 {
        return Err("a text length is 0, but each length counts its NUL".to_string());
    }
    let host_end = HEADER_BYTES.saturating_add(host_len);
    let name_end = host_end.saturating_add(name_len);
    let (Some(host), Some(name)) = (
        bytes.get(HEADER_BYTES..host_end),
        bytes.get(host_end..name_end),
    ) else {
        return Err(format!(
            "the host and name lengths need {name_end} bytes, but the entry holds {}",
            bytes.len()
        ));
    };
    let flags = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    Ok(Route {
        name: text(name, "name")?,
        ams_net_id: format!(
            "{}.{}.{}.{}.{}.{}",
            header[0], header[1], header[2], header[3], header[4], header[5]
        ),
        host: text(host, "host")?,
        temporary: (flags & 0x01) != 0,
        unidirectional: (flags & 0x80) != 0,
        virtual_nat: (flags & 0x100) != 0,
    })
}

/// A little-endian length at `offset` of the header. A length that does not fit a `usize` is
/// returned as `usize::MAX`, which the bounds check then reports as damage.
fn header_len(header: &[u8; HEADER_BYTES], offset: usize) -> usize {
    let value = u32::from_le_bytes([
        header[offset],
        header[offset + 1],
        header[offset + 2],
        header[offset + 3],
    ]);
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// The text of one field: it ends with the NUL that its length counts, and holds no other NUL. The
/// text is Windows-1252, the Windows ANSI code page that TwinCAT keeps its texts in, so each byte
/// is one character.
fn text(bytes: &[u8], field: &str) -> Result<String, String> {
    let Some((&0, body)) = bytes.split_last() else {
        return Err(format!("the {field} text does not end with its NUL"));
    };
    if body.contains(&0) {
        return Err(format!("the {field} text holds a NUL before its end"));
    }
    // Windows-1252 has a character for every byte, so the decoder replaces nothing.
    let (decoded, _) = WINDOWS_1252.decode_without_bom_handling(body);
    Ok(decoded.into_owned())
}

fn unavailable(reason: &str, error: Option<String>) -> RouteList {
    RouteList::Unavailable {
        reason: reason.to_string(),
        error,
    }
}

fn deadline_passed() -> RouteList {
    unavailable(
        "The search deadline passed before the route list ended.",
        None,
    )
}

#[cfg(test)]
#[path = "ads_routes_tests.rs"]
mod tests;
