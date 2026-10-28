//! The portable I/O snapshot (specification 34, ARCH-11; specification 11, "Live Values").
//!
//! `io.list` and `io.read` answer with this snapshot. It carries the same fields and renders the
//! same JSON as trust-runtime's hosted snapshot, but names each entry's declared type with the
//! core's IEC type name instead of a `trust_hir::TypeId`, so a target without the compiler's type
//! table can hold it.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use serde::ser::{SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};
use smol_str::SmolStr;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::io_value_type_name;
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
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

impl Serialize for IoSnapshot {
    /// The snapshot as `io.list` and `io.read` answer it, written without building a value, its
    /// members in the order of their names as a value's object writes them (specification 34
    /// PORT-02).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut snapshot = serializer.serialize_struct("IoSnapshot", 4)?;
        snapshot.serialize_field("inputs", &Entries::of(&self.inputs, &self.forced))?;
        snapshot.serialize_field("memory", &Entries::of(&self.memory, &self.forced))?;
        snapshot.serialize_field("outputs", &Entries::of(&self.outputs, &self.forced))?;
        snapshot.serialize_field("scan", &self.scan)?;
        snapshot.end()
    }
}

/// The IEC name of a scalar I/O type, or `None` for a type that has no fixed image width
/// (`DATE`, `TOD` and `DT` in IEC 61131-3 Table 10).
#[must_use]
pub fn value_type_name(wire_type: ConversionType) -> Option<&'static str> {
    io_value_type_name(wire_type)
}

/// The entries of one area, each with whether a force holds its address.
struct Entries<'a> {
    entries: &'a [IoSnapshotEntry],
    forced: &'a [IoAddress],
}

impl<'a> Entries<'a> {
    fn of(entries: &'a [IoSnapshotEntry], forced: &'a [IoAddress]) -> Self {
        Self { entries, forced }
    }
}

impl Serialize for Entries<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut entries = serializer.serialize_seq(Some(self.entries.len()))?;
        for entry in self.entries {
            entries.serialize_element(&Entry {
                address: AddressText(&entry.address),
                forced: self.forced.iter().any(|address| address == &entry.address),
                name: entry.name.as_ref().map(SmolStr::as_str),
                source: entry.source.as_ref().map(SmolStr::as_str),
                value: &entry.value,
                value_type: entry.value_type_name.as_deref(),
            })?;
        }
        entries.end()
    }
}

/// One entry: its name, address, value and force mark, and its declared type name and its source
/// when it has them.
#[derive(Serialize)]
struct Entry<'a> {
    address: AddressText<'a>,
    forced: bool,
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'a str>,
    value: &'a IoSnapshotValue,
    #[serde(rename = "valueType", skip_serializing_if = "Option::is_none")]
    value_type: Option<&'a str>,
}

impl Serialize for IoSnapshotValue {
    /// A sampled value as trust-runtime reports it today: the value's debug text, an object with
    /// the conversion's error, or `unresolved`.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            IoSnapshotValue::Value(value) => serializer.collect_str(&DebugText(value)),
            IoSnapshotValue::Error(error) => {
                let mut object = serializer.serialize_struct("IoSnapshotError", 1)?;
                object.serialize_field("error", error)?;
                object.end()
            }
            IoSnapshotValue::Unresolved => serializer.serialize_str("unresolved"),
        }
    }
}

/// A value written as its debug text, `Bool(true)`.
struct DebugText<'a>(&'a Value);

impl fmt::Display for DebugText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}

/// An address as its IEC text (`%IX0.0`, `%QW2`, `%M*` for a wildcard), as `io.list` and `io.read`
/// report it, written straight into the reply.
struct AddressText<'a>(&'a IoAddress);

impl Serialize for AddressText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self.0)
    }
}

#[cfg(test)]
mod tests;
