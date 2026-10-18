//! The portable I/O snapshot (specification 34, ARCH-11; specification 11, "Live Values").
//!
//! `io.list` and `io.read` answer with this snapshot. It carries the same fields and renders the
//! same JSON as trust-runtime's hosted snapshot, but names each entry's declared type with the
//! core's IEC type name instead of a `trust_hir::TypeId`, so a target without the compiler's type
//! table can hold it.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::io_image::io_value_type_name;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::stdlib::conversions::ConversionType;
use trust_runtime_core::value::Value;

/// The sampled value of one I/O snapshot entry.
#[derive(Debug, Clone)]
pub enum IoSnapshotValue {
    /// The value read from the process image.
    Value(Value),
    /// The image value could not be converted to the declared type.
    Error(String),
    /// The address is a wildcard that configuration has not resolved.
    Unresolved,
}

/// One entry of the portable I/O snapshot.
#[derive(Debug, Clone)]
pub struct IoSnapshotEntry {
    /// Bound name, when the binding has one.
    pub name: Option<SmolStr>,
    /// Direct address of the entry.
    pub address: IoAddress,
    /// IEC name of the declared type, when the binding is typed.
    pub value_type_name: Option<SmolStr>,
    /// The sampled value.
    pub value: IoSnapshotValue,
    /// Source of the value, when the platform reports one.
    pub source: Option<SmolStr>,
}

/// The portable I/O snapshot: the scan number, the addresses an active force holds and the entries
/// of each process-image area.
#[derive(Debug, Clone, Default)]
pub struct IoSnapshot {
    /// Scan number the snapshot was taken in.
    pub scan: Option<u64>,
    /// Addresses an active force holds.
    pub forced: Vec<IoAddress>,
    /// Input-area entries.
    pub inputs: Vec<IoSnapshotEntry>,
    /// Output-area entries.
    pub outputs: Vec<IoSnapshotEntry>,
    /// Memory-area entries.
    pub memory: Vec<IoSnapshotEntry>,
}

impl IoSnapshot {
    /// The JSON `io.list` and `io.read` answer: the scan number and, for every entry, its name,
    /// address, value, force mark, declared type name and source.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "scan": self.scan,
            "inputs": self.inputs.iter().map(|entry| entry_to_json(entry, &self.forced)).collect::<Vec<_>>(),
            "outputs": self.outputs.iter().map(|entry| entry_to_json(entry, &self.forced)).collect::<Vec<_>>(),
            "memory": self.memory.iter().map(|entry| entry_to_json(entry, &self.forced)).collect::<Vec<_>>(),
        })
    }
}

/// The IEC name of a scalar I/O type, or `None` for a type that has no fixed image width
/// (`DATE`, `TOD` and `DT` in IEC 61131-3 Table 10).
#[must_use]
pub fn value_type_name(wire_type: ConversionType) -> Option<&'static str> {
    io_value_type_name(wire_type)
}

/// The `valueType` JSON of one entry: the name is set only when the binding is typed.
fn entry_to_json(entry: &IoSnapshotEntry, forced: &[IoAddress]) -> serde_json::Value {
    let mut payload = json!({
        "name": entry.name.as_ref().map(SmolStr::as_str),
        "address": format_address(&entry.address),
        "value": format_value(&entry.value),
        "forced": forced.iter().any(|address| address == &entry.address),
    });
    if let Some(value_type) = entry.value_type_name.as_deref() {
        payload["valueType"] = json!(value_type);
    }
    if let Some(source) = &entry.source {
        payload["source"] = json!(source.as_str());
    }
    payload
}

/// The JSON of a sampled value, as trust-runtime reports it today.
fn format_value(value: &IoSnapshotValue) -> serde_json::Value {
    match value {
        IoSnapshotValue::Value(value) => json!(format!("{value:?}")),
        IoSnapshotValue::Error(error) => json!({ "error": error }),
        IoSnapshotValue::Unresolved => json!("unresolved"),
    }
}

/// The IEC direct-address text of `address`, as `io.list` and `io.read` report it today.
fn format_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte | IoSize::Bytes(_) => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    let mut path = address
        .path
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(".");
    if matches!(address.size, IoSize::Bit) {
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(&address.bit.to_string());
    }
    if path.is_empty() {
        format!("%{area}{size}{}", address.byte)
    } else {
        format!("%{area}{size}{path}")
    }
}

#[cfg(test)]
mod tests;
