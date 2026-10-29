//! The I/O state as Live Values shows it (specification 25 §4.3): the event `stIoState` carries it.
//! The runtime's `io.read` writes each value in Rust's debug form (`Bool(true)`, `DInt(5)`); here a
//! truth value becomes `TRUE` or `FALSE` and a number its digits, a value type is written in
//! capitals, and an entry whose type the runtime does not name has the type its direct address
//! names by its size; an entry without an address or a value is left out, and the globals that an
//! ADS tag binds (a `global:` address whose source begins with `ADS`) are listed apart from the
//! process image's inputs, outputs and memory.

use serde::Serialize;
use trust_runtime::io::{IoAddress, IoSize};

use crate::protocol::{IoStateEntry, IoStateEventBody};

/// The I/O state of one scan, as Live Values shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DisplayIoState {
    /// The scan the values are from, when the runtime names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scan: Option<u64>,
    /// The inputs.
    pub(crate) inputs: Vec<IoStateEntry>,
    /// The outputs.
    pub(crate) outputs: Vec<IoStateEntry>,
    /// The memory.
    pub(crate) memory: Vec<IoStateEntry>,
    /// The globals that ADS tags bind.
    pub(crate) ads: Vec<IoStateEntry>,
}

/// `body` as Live Values shows it.
pub(crate) fn display_io_state(body: &IoStateEventBody) -> DisplayIoState {
    let mut ads = Vec::new();
    let mut part = |entries: &[IoStateEntry]| {
        let mut kept = Vec::new();
        for entry in entries.iter().filter_map(display_entry) {
            if is_ads_global(&entry) {
                ads.push(entry);
            } else {
                kept.push(entry);
            }
        }
        kept
    };
    let inputs = part(&body.inputs);
    let outputs = part(&body.outputs);
    let memory = part(&body.memory);
    DisplayIoState {
        scan: body.scan,
        inputs,
        outputs,
        memory,
        ads,
    }
}

/// `entry` as Live Values shows it; `None` without an address or a value.
fn display_entry(entry: &IoStateEntry) -> Option<IoStateEntry> {
    if entry.address.is_empty() || entry.value.is_empty() {
        return None;
    }
    Some(IoStateEntry {
        name: entry.name.clone(),
        address: entry.address.clone(),
        source: entry.source.clone(),
        value_type: entry
            .value_type
            .as_deref()
            .map(str::to_ascii_uppercase)
            .or_else(|| address_type(&entry.address)),
        value: display_value(&entry.value),
        forced: entry.forced,
        writable: entry.writable,
    })
}

/// The type that a direct address names by its size (IEC 61131-3 §6.5.5, Table 16): `X` or no size
/// prefix `BOOL`, `B` `BYTE`, `W` `WORD`, `D` `DWORD`, `L` `LWORD`; none for a byte window or a
/// text that is no direct address, such as `global:Speed`.
fn address_type(address: &str) -> Option<String> {
    let name = match IoAddress::parse(address).ok()?.size {
        IoSize::Bit => "BOOL",
        IoSize::Byte => "BYTE",
        IoSize::Word => "WORD",
        IoSize::DWord => "DWORD",
        IoSize::LWord => "LWORD",
        IoSize::Bytes(_) => return None,
    };
    Some(name.to_owned())
}

/// The numeric value kinds whose debug form `Kind(digits)` shows as its digits.
const NUMERIC_KINDS: [&str; 14] = [
    "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL", "BYTE",
    "WORD", "DWORD", "LWORD",
];

/// A value in IEC text: `Bool(true)` as `TRUE`, `DInt(5)` as `5`; any other value as it is.
fn display_value(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some((kind, rest)) = trimmed.split_once('(') else {
        return raw.to_owned();
    };
    let Some(inner) = rest.strip_suffix(')') else {
        return raw.to_owned();
    };
    let kind = kind.to_ascii_uppercase();
    if kind == "BOOL" {
        if inner.eq_ignore_ascii_case("true") {
            return "TRUE".to_owned();
        }
        if inner.eq_ignore_ascii_case("false") {
            return "FALSE".to_owned();
        }
    }
    if NUMERIC_KINDS.contains(&kind.as_str()) {
        return inner.to_owned();
    }
    raw.to_owned()
}

/// Whether `entry` is a global that an ADS tag binds.
fn is_ads_global(entry: &IoStateEntry) -> bool {
    let source = entry.source.as_deref().unwrap_or_default().trim();
    let ads = source
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("ADS"))
        && source[3..].chars().next().is_none_or(char::is_whitespace);
    entry.address.starts_with("global:") && ads
}

#[cfg(test)]
#[path = "io_values_tests.rs"]
mod tests;
