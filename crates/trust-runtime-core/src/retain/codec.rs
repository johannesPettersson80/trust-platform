//! The STRN v3 retain payload codec, shared by computers and chips (specification 11 §6.7,
//! specification 34 §10.1).
//!
//! A retain file is the magic `STRN`, a version, the payload length, the payload, a payload
//! CRC-32 and the trailer `ENDR`. The v3 payload is a count followed by a name and a tagged value
//! for each retained variable; a v3 structure also carries its opaque physical checkpoint. A
//! chip's checkpoint records carry exactly these payload bytes (specification 34 §10.1), so a chip
//! and a computer write and read the same snapshot bytes with this one codec.
//!
//! Versions 1 and 2 are still read: they store the same payload without the physical checkpoint
//! and, for version 1, without the length, payload checksum and trailer.

mod reader;
mod view;

use alloc::{boxed::Box, format, sync::Arc, vec::Vec};

use smol_str::SmolStr;

use crate::collections::OrderedMap as IndexMap;
use crate::error::RuntimeError;
use crate::memory::view::ValueView;
use crate::retain::RetainSnapshot;
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};

use self::reader::RetainReader;
use self::view::encode_view;

pub use self::reader::encode_string;
pub(crate) use self::view::view_is_retainable;

/// The four-byte magic at the start of every retain file.
const RETAIN_MAGIC: &[u8; 4] = b"STRN";
/// The version of a bare payload with no length, payload checksum or trailer.
const RETAIN_V1_VERSION: u16 = 1;
/// The version of a length-and-checksum container of a payload without physical checkpoints.
const RETAIN_V2_VERSION: u16 = 2;
/// The current version, whose structures may carry a physical checkpoint.
const RETAIN_VERSION: u16 = 3;
/// The four-byte trailer at the end of a v2 and v3 retain file.
const RETAIN_TRAILER: &[u8; 4] = b"ENDR";

/// Encode a snapshot as a complete STRN v3 retain file.
///
/// # Errors
///
/// [`RuntimeError::RetainStore`] when a value cannot be persisted: a non-finite `REAL` or `LREAL`,
/// a reference or instance value, or a physical checkpoint that is too large.
pub fn encode_snapshot(snapshot: &RetainSnapshot) -> Result<Vec<u8>, RuntimeError> {
    Ok(frame_payload(&encode_snapshot_payload(snapshot)?))
}

/// The complete STRN v3 retain file of a payload a capture staged: its magic, version, length and
/// checksum around it (specification 11 §6.7). A compiled program's capture writes the payload from
/// its retain area (specification 44 §11), so a computer frames it without decoding it.
#[must_use]
pub fn frame_payload(payload: &[u8]) -> Vec<u8> {
    let checksum = crate::crc32::checksum(payload);
    let mut out = Vec::with_capacity(4 + 2 + 8 + payload.len() + 4 + 4);
    out.extend_from_slice(RETAIN_MAGIC);
    out.extend_from_slice(&RETAIN_VERSION.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(RETAIN_TRAILER);
    out
}

fn encode_snapshot_payload(snapshot: &RetainSnapshot) -> Result<Vec<u8>, RuntimeError> {
    let mut out = Vec::new();
    snapshot.encode_snapshot_payload_into(&mut out)?;
    Ok(out)
}

impl RetainSnapshot {
    /// Append this snapshot's STRN v3 payload bytes to `out`, keeping `out`'s capacity.
    ///
    /// The bytes are exactly the payload [`encode_snapshot`] frames. A capture clears and refills
    /// one reserved buffer through this and [`PayloadWriter`], so a steady-state capture allocates
    /// nothing (specification 11 §6.7, specification 34 MEM-04).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when a value cannot be persisted: a non-finite `REAL` or
    /// `LREAL`, a reference or instance value, or an oversized physical checkpoint.
    pub fn encode_snapshot_payload_into(&self, out: &mut Vec<u8>) -> Result<(), RuntimeError> {
        let mut writer = PayloadWriter::new(out);
        for (name, value) in self.values() {
            writer.entry(name.as_str(), value)?;
        }
        writer.finish();
        Ok(())
    }
}

/// Appends STRN v3 payload entries to a caller's buffer (specification 11 §6.7).
///
/// A writer starts at the buffer's current end, writes an entry-count placeholder, appends one
/// name and tagged value per entry, and patches the count when it is
/// [`finish`](Self::finish)ed. The buffer stays the caller's own, so a capture that clears and
/// refills it allocates nothing in steady state (specification 34 MEM-04).
pub struct PayloadWriter<'a> {
    out: &'a mut Vec<u8>,
    start: usize,
    count: u32,
}

impl<'a> PayloadWriter<'a> {
    /// Begin a payload at the current end of `out`.
    #[must_use]
    pub fn new(out: &'a mut Vec<u8>) -> Self {
        let start = out.len();
        out.extend_from_slice(&0_u32.to_le_bytes());
        Self {
            out,
            start,
            count: 0,
        }
    }

    /// Append one named retained value.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when the value cannot be persisted: a non-finite `REAL` or
    /// `LREAL`, a reference or instance value, or an oversized physical checkpoint.
    pub fn entry(&mut self, name: &str, value: &Value) -> Result<(), RuntimeError> {
        encode_string(self.out, name);
        encode_value(self.out, value)?;
        self.count += 1;
        Ok(())
    }

    /// Append one retained value whose name is the concatenation of `pieces`, written as pieces
    /// so the caller builds no key string (specification 11 §6.7).
    ///
    /// # Errors
    ///
    /// As [`entry`](Self::entry).
    pub fn entry_pieces(&mut self, pieces: &[&str], value: &Value) -> Result<(), RuntimeError> {
        self.encode_name_pieces(pieces);
        encode_value(self.out, value)?;
        self.count += 1;
        Ok(())
    }

    /// Append one named retained value that the store holds as `view`: a view of the `Value` tree is
    /// encoded as [`entry`](Self::entry) encodes it, and a view of cells is encoded from its cells, its
    /// text buffers and its compound members without building a `Value`, to the same bytes
    /// (specification 11 §6.7; C1-9d2b-2).
    ///
    /// # Errors
    ///
    /// As [`entry`](Self::entry), and the refusal of a cell the view's type does not hold.
    pub(crate) fn entry_view(
        &mut self,
        name: &str,
        view: &ValueView<'_>,
    ) -> Result<(), RuntimeError> {
        encode_string(self.out, name);
        encode_view(self.out, view)?;
        self.count += 1;
        Ok(())
    }

    /// Append one retained value held as `view`, whose name is the concatenation of `pieces`, as
    /// [`entry_pieces`](Self::entry_pieces) appends a `Value`.
    ///
    /// # Errors
    ///
    /// As [`entry_view`](Self::entry_view).
    pub(crate) fn entry_view_pieces(
        &mut self,
        pieces: &[&str],
        view: &ValueView<'_>,
    ) -> Result<(), RuntimeError> {
        self.encode_name_pieces(pieces);
        encode_view(self.out, view)?;
        self.count += 1;
        Ok(())
    }

    /// Append one retained value whose name is the concatenation of `pieces` and whose tagged value
    /// `encode` appends, for a value held in neither a `Value` nor cells: a compiled program writes
    /// it from its retain area through its type table (specification 44 §11). `encode` must append
    /// exactly the bytes [`entry`](Self::entry) writes for the same value.
    ///
    /// # Errors
    ///
    /// The error of `encode`; the payload then holds a partial entry, which the capture discards.
    pub(crate) fn entry_with(
        &mut self,
        pieces: &[&str],
        encode: impl FnOnce(&mut Vec<u8>) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        self.encode_name_pieces(pieces);
        encode(self.out)?;
        self.count += 1;
        Ok(())
    }

    /// Write an entry name given as pieces: the length of all of them, then each piece.
    fn encode_name_pieces(&mut self, pieces: &[&str]) {
        let length: usize = pieces.iter().map(|piece| piece.len()).sum();
        self.out.extend_from_slice(&(length as u32).to_le_bytes());
        for piece in pieces {
            self.out.extend_from_slice(piece.as_bytes());
        }
    }

    /// Finish the payload, writing its entry count.
    pub fn finish(self) {
        self.out[self.start..self.start + 4].copy_from_slice(&self.count.to_le_bytes());
    }
}

/// Decode a retain file of version 1, 2 or 3.
///
/// # Errors
///
/// [`RuntimeError::RetainCorruption`] when the magic, version, framing, trailer or payload
/// checksum is invalid, when the payload is malformed, or when a decoded value is not a valid
/// runtime value.
pub fn decode_snapshot(bytes: &[u8]) -> Result<RetainSnapshot, RuntimeError> {
    let mut reader = RetainReader::new(bytes);
    let magic = reader.read_bytes(4)?;
    if magic != RETAIN_MAGIC {
        return Err(RuntimeError::RetainCorruption(
            "invalid retain magic".into(),
        ));
    }
    let version = reader.read_u16()?;
    match version {
        RETAIN_V1_VERSION => {
            let snapshot = decode_snapshot_payload(&mut reader, false)?;
            reader.expect_finished("retain v1 payload")?;
            Ok(snapshot)
        }
        RETAIN_V2_VERSION => decode_snapshot_v2(&mut reader),
        RETAIN_VERSION => decode_snapshot_v3(&mut reader),
        _ => Err(RuntimeError::RetainCorruption(
            format!("unsupported retain version {version}").into(),
        )),
    }
}

fn decode_snapshot_v2(reader: &mut RetainReader<'_>) -> Result<RetainSnapshot, RuntimeError> {
    let payload = read_checksummed_payload(reader, RETAIN_V2_VERSION)?;
    let mut payload_reader = RetainReader::new(payload);
    let snapshot = decode_snapshot_payload(&mut payload_reader, false)?;
    payload_reader.expect_finished("retain v2 payload")?;
    Ok(snapshot)
}

fn decode_snapshot_v3(reader: &mut RetainReader<'_>) -> Result<RetainSnapshot, RuntimeError> {
    let payload = read_checksummed_payload(reader, RETAIN_VERSION)?;
    decode_snapshot_payload_v3(payload)
}

/// Read a v2 or v3 container: its payload length, its payload, the payload's CRC-32 and the
/// trailer; check the framing and the payload's checksum, and return the payload bytes.
fn read_checksummed_payload<'r>(
    reader: &mut RetainReader<'r>,
    version: u16,
) -> Result<&'r [u8], RuntimeError> {
    let payload_len = reader.read_u64()?;
    let payload_len: usize = payload_len.try_into().map_err(|_| {
        RuntimeError::RetainCorruption("retain payload length exceeds platform size".into())
    })?;
    let payload = reader.read_bytes(payload_len)?;
    let expected_checksum = reader.read_u32()?;
    let trailer = reader.read_bytes(RETAIN_TRAILER.len())?;
    if trailer != RETAIN_TRAILER {
        return Err(RuntimeError::RetainCorruption(
            "invalid retain trailer".into(),
        ));
    }
    reader.expect_finished(match version {
        RETAIN_V2_VERSION => "retain v2 container",
        RETAIN_VERSION => "retain v3 container",
        _ => {
            return Err(RuntimeError::RetainCorruption(
                "invalid retain version".into(),
            ))
        }
    })?;
    let actual_checksum = crate::crc32::checksum(payload);
    if actual_checksum != expected_checksum {
        return Err(RuntimeError::RetainCorruption(
            format!(
                "retain checksum mismatch expected {expected_checksum:#010x} got {actual_checksum:#010x}"
            )
            .into(),
        ));
    }
    Ok(payload)
}

fn decode_snapshot_payload(
    reader: &mut RetainReader<'_>,
    physical_checkpoints: bool,
) -> Result<RetainSnapshot, RuntimeError> {
    let count = reader.read_u32()? as usize;
    let mut values = IndexMap::default();
    for _ in 0..count {
        let name = SmolStr::new(reader.read_string()?);
        let value = decode_value(reader, physical_checkpoints)?;
        values.insert(name, value);
    }
    Ok(RetainSnapshot::from_values(values))
}

/// Decode a bare STRN v3 payload: exactly the bytes
/// [`RetainSnapshot::encode_snapshot_payload_into`] appends, without the file's magic, version,
/// length, payload checksum and trailer (specification 34 §10.1).
///
/// [`decode_snapshot`] checks the framing of a v3 file and then applies this payload rule, and a
/// chip applies it to the payload its checkpoint log returned, because a checkpoint record
/// carries the payload bytes alone (specification 34 §10.1).
///
/// # Errors
///
/// [`RuntimeError::RetainCorruption`] when the payload is malformed or has trailing data, or when
/// a decoded value is not a valid runtime value.
pub fn decode_snapshot_payload_v3(payload: &[u8]) -> Result<RetainSnapshot, RuntimeError> {
    let mut reader = RetainReader::new(payload);
    let snapshot = decode_snapshot_payload(&mut reader, true)?;
    reader.expect_finished("retain v3 payload")?;
    Ok(snapshot)
}

/// The same payload rule under the name the shared VM engine's explicit checkpoint asks for
/// (`vm/engine/cycle.rs`); it is the one decoder, not a second one.
pub(crate) use self::decode_snapshot_payload_v3 as decode_snapshot_payload_bytes;

/// The one-byte wire tag that precedes every retained value.
///
/// The discriminants are the codec's wire format and must not change.
#[derive(Debug, Clone, Copy)]
pub enum ValueTag {
    /// A `BOOL`.
    Bool = 1,
    /// A `SINT`.
    SInt = 2,
    /// An `INT`.
    Int = 3,
    /// A `DINT`.
    DInt = 4,
    /// A `LINT`.
    LInt = 5,
    /// A `USINT`.
    USInt = 6,
    /// A `UINT`.
    UInt = 7,
    /// A `UDINT`.
    UDInt = 8,
    /// A `ULINT`.
    ULInt = 9,
    /// A `REAL`.
    Real = 10,
    /// A `LREAL`.
    LReal = 11,
    /// A `BYTE`.
    Byte = 12,
    /// A `WORD`.
    Word = 13,
    /// A `DWORD`.
    DWord = 14,
    /// A `LWORD`.
    LWord = 15,
    /// A `TIME`.
    Time = 16,
    /// An `LTIME`.
    LTime = 17,
    /// A `DATE`.
    Date = 18,
    /// An `LDATE`.
    LDate = 19,
    /// A `TIME_OF_DAY`.
    Tod = 20,
    /// An `LTIME_OF_DAY`.
    LTod = 21,
    /// A `DATE_AND_TIME`.
    Dt = 22,
    /// An `LDATE_AND_TIME`.
    Ldt = 23,
    /// A `STRING`.
    String = 24,
    /// A `WSTRING`.
    WString = 25,
    /// A `CHAR`.
    Char = 26,
    /// A `WCHAR`.
    WChar = 27,
    /// An array.
    Array = 28,
    /// A structure or union.
    Struct = 29,
    /// An enumeration value.
    Enum = 30,
    /// The null value.
    Null = 31,
}

fn encode_value(out: &mut Vec<u8>, value: &Value) -> Result<(), RuntimeError> {
    match value {
        Value::Bool(v) => {
            out.push(ValueTag::Bool as u8);
            out.push(u8::from(*v));
        }
        Value::SInt(v) => {
            out.push(ValueTag::SInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Int(v) => {
            out.push(ValueTag::Int as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::DInt(v) => {
            out.push(ValueTag::DInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::LInt(v) => {
            out.push(ValueTag::LInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::USInt(v) => {
            out.push(ValueTag::USInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::UInt(v) => {
            out.push(ValueTag::UInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::UDInt(v) => {
            out.push(ValueTag::UDInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::ULInt(v) => {
            out.push(ValueTag::ULInt as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Real(v) => {
            if !v.is_finite() {
                return Err(RuntimeError::RetainStore(
                    "cannot persist non-finite REAL retain value".into(),
                ));
            }
            out.push(ValueTag::Real as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::LReal(v) => {
            if !v.is_finite() {
                return Err(RuntimeError::RetainStore(
                    "cannot persist non-finite LREAL retain value".into(),
                ));
            }
            out.push(ValueTag::LReal as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Byte(v) => {
            out.push(ValueTag::Byte as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Word(v) => {
            out.push(ValueTag::Word as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::DWord(v) => {
            out.push(ValueTag::DWord as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::LWord(v) => {
            out.push(ValueTag::LWord as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Time(v) => {
            out.push(ValueTag::Time as u8);
            out.extend_from_slice(&v.as_nanos().to_le_bytes());
        }
        Value::LTime(v) => {
            out.push(ValueTag::LTime as u8);
            out.extend_from_slice(&v.as_nanos().to_le_bytes());
        }
        Value::Date(v) => {
            out.push(ValueTag::Date as u8);
            out.extend_from_slice(&v.ticks().to_le_bytes());
        }
        Value::LDate(v) => {
            out.push(ValueTag::LDate as u8);
            out.extend_from_slice(&v.nanos().to_le_bytes());
        }
        Value::Tod(v) => {
            out.push(ValueTag::Tod as u8);
            out.extend_from_slice(&v.ticks().to_le_bytes());
        }
        Value::LTod(v) => {
            out.push(ValueTag::LTod as u8);
            out.extend_from_slice(&v.nanos().to_le_bytes());
        }
        Value::Dt(v) => {
            out.push(ValueTag::Dt as u8);
            out.extend_from_slice(&v.ticks().to_le_bytes());
        }
        Value::Ldt(v) => {
            out.push(ValueTag::Ldt as u8);
            out.extend_from_slice(&v.nanos().to_le_bytes());
        }
        Value::String(v) => {
            out.push(ValueTag::String as u8);
            encode_string(out, v.as_str());
        }
        Value::WString(v) => {
            out.push(ValueTag::WString as u8);
            encode_string(out, v);
        }
        Value::Char(v) => {
            out.push(ValueTag::Char as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::WChar(v) => {
            out.push(ValueTag::WChar as u8);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Array(array) => {
            encode_array_header(
                out,
                array.elements().len(),
                array.dimensions().iter().copied(),
            );
            for element in array.elements() {
                encode_value(out, element)?;
            }
        }
        Value::Struct(struct_value) => {
            encode_struct_header(
                out,
                struct_value.type_name().as_str(),
                struct_value.fields().len(),
            );
            for (name, field) in struct_value.fields() {
                encode_string(out, name.as_str());
                encode_value(out, field)?;
            }
            match struct_value.encode_physical_checkpoint().map_err(|error| {
                RuntimeError::RetainStore(
                    format!("cannot encode physical retain checkpoint: {error}").into(),
                )
            })? {
                Some(checkpoint) => {
                    out.push(PHYSICAL_CHECKPOINT);
                    let len = u32::try_from(checkpoint.len()).map_err(|_| {
                        RuntimeError::RetainStore("physical retain checkpoint is too large".into())
                    })?;
                    out.extend_from_slice(&len.to_le_bytes());
                    out.extend_from_slice(&checkpoint);
                }
                None => out.push(NO_PHYSICAL_CHECKPOINT),
            }
        }
        Value::Enum(enum_value) => encode_enumeration(
            out,
            enum_value.type_name().as_str(),
            enum_value.variant_name().as_str(),
            enum_value.numeric_value(),
        ),
        Value::Null => {
            out.push(ValueTag::Null as u8);
        }
        Value::Reference(_) | Value::Instance(_) => return Err(reference_or_instance_refused()),
    }
    Ok(())
}

/// The marker that ends a structure value without a physical checkpoint.
pub(crate) const NO_PHYSICAL_CHECKPOINT: u8 = 0;
/// The marker that precedes a structure value's physical checkpoint.
const PHYSICAL_CHECKPOINT: u8 = 1;

/// Write the header of an array of `elements` elements within `dimensions`: its tag, its element count,
/// its dimension count and the inclusive bounds of each dimension. The elements follow.
pub(crate) fn encode_array_header(
    out: &mut Vec<u8>,
    elements: usize,
    dimensions: impl ExactSizeIterator<Item = (i64, i64)>,
) {
    out.push(ValueTag::Array as u8);
    out.extend_from_slice(&(elements as u32).to_le_bytes());
    out.extend_from_slice(&(dimensions.len() as u32).to_le_bytes());
    for (lower, upper) in dimensions {
        out.extend_from_slice(&lower.to_le_bytes());
        out.extend_from_slice(&upper.to_le_bytes());
    }
}

/// Write the header of a structure of `members` members: its tag, its type name and its member count.
/// Each member's name and value follow, then the marker for its physical checkpoint.
pub(crate) fn encode_struct_header(out: &mut Vec<u8>, type_name: &str, members: usize) {
    out.push(ValueTag::Struct as u8);
    encode_string(out, type_name);
    out.extend_from_slice(&(members as u32).to_le_bytes());
}

/// Write an enumeration value: its tag, its type name, its variant name and its numeric value.
pub(crate) fn encode_enumeration(
    out: &mut Vec<u8>,
    type_name: &str,
    variant_name: &str,
    numeric: i64,
) {
    out.push(ValueTag::Enum as u8);
    encode_string(out, type_name);
    encode_string(out, variant_name);
    out.extend_from_slice(&numeric.to_le_bytes());
}

/// The refusal of a value that holds a live handle, which no snapshot persists.
fn reference_or_instance_refused() -> RuntimeError {
    RuntimeError::RetainStore("cannot retain reference/instance values".into())
}

fn decode_value(
    reader: &mut RetainReader<'_>,
    physical_checkpoints: bool,
) -> Result<Value, RuntimeError> {
    let tag = reader.read_u8()?;
    let value = match tag {
        x if x == ValueTag::Bool as u8 => Value::Bool(reader.read_u8()? != 0),
        x if x == ValueTag::SInt as u8 => Value::SInt(reader.read_i8()?),
        x if x == ValueTag::Int as u8 => Value::Int(reader.read_i16()?),
        x if x == ValueTag::DInt as u8 => Value::DInt(reader.read_i32()?),
        x if x == ValueTag::LInt as u8 => Value::LInt(reader.read_i64()?),
        x if x == ValueTag::USInt as u8 => Value::USInt(reader.read_u8()?),
        x if x == ValueTag::UInt as u8 => Value::UInt(reader.read_u16()?),
        x if x == ValueTag::UDInt as u8 => Value::UDInt(reader.read_u32()?),
        x if x == ValueTag::ULInt as u8 => Value::ULInt(reader.read_u64()?),
        x if x == ValueTag::Real as u8 => {
            let value = reader.read_f32()?;
            if !value.is_finite() {
                return Err(RuntimeError::RetainCorruption(
                    "non-finite REAL in retain snapshot".into(),
                ));
            }
            Value::Real(value)
        }
        x if x == ValueTag::LReal as u8 => {
            let value = reader.read_f64()?;
            if !value.is_finite() {
                return Err(RuntimeError::RetainCorruption(
                    "non-finite LREAL in retain snapshot".into(),
                ));
            }
            Value::LReal(value)
        }
        x if x == ValueTag::Byte as u8 => Value::Byte(reader.read_u8()?),
        x if x == ValueTag::Word as u8 => Value::Word(reader.read_u16()?),
        x if x == ValueTag::DWord as u8 => Value::DWord(reader.read_u32()?),
        x if x == ValueTag::LWord as u8 => Value::LWord(reader.read_u64()?),
        x if x == ValueTag::Time as u8 => Value::Time(Duration::from_nanos(reader.read_i64()?)),
        x if x == ValueTag::LTime as u8 => Value::LTime(Duration::from_nanos(reader.read_i64()?)),
        x if x == ValueTag::Date as u8 => Value::Date(DateValue::new(reader.read_i64()?)),
        x if x == ValueTag::LDate as u8 => Value::LDate(LDateValue::new(reader.read_i64()?)),
        x if x == ValueTag::Tod as u8 => Value::Tod(TimeOfDayValue::new(reader.read_i64()?)),
        x if x == ValueTag::LTod as u8 => Value::LTod(LTimeOfDayValue::new(reader.read_i64()?)),
        x if x == ValueTag::Dt as u8 => Value::Dt(DateTimeValue::new(reader.read_i64()?)),
        x if x == ValueTag::Ldt as u8 => Value::Ldt(LDateTimeValue::new(reader.read_i64()?)),
        x if x == ValueTag::String as u8 => Value::String(SmolStr::new(reader.read_string()?)),
        x if x == ValueTag::WString as u8 => Value::WString(reader.read_string()?),
        x if x == ValueTag::Char as u8 => Value::Char(reader.read_u8()?),
        x if x == ValueTag::WChar as u8 => Value::WChar(reader.read_u16()?),
        x if x == ValueTag::Array as u8 => {
            let len = reader.read_u32()? as usize;
            let dims = reader.read_u32()? as usize;
            let mut dimensions = Vec::with_capacity(dims);
            for _ in 0..dims {
                dimensions.push((reader.read_i64()?, reader.read_i64()?));
            }
            let mut elements = Vec::with_capacity(len);
            for _ in 0..len {
                elements.push(decode_value(reader, physical_checkpoints)?);
            }
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(elements, dimensions).map_err(|error| {
                    RuntimeError::RetainStore(
                        format!("invalid retained array value: {error}").into(),
                    )
                })?,
            ))
        }
        x if x == ValueTag::Struct as u8 => {
            let type_name = SmolStr::new(reader.read_string()?);
            let count = reader.read_u32()? as usize;
            let mut fields = IndexMap::default();
            for _ in 0..count {
                let name = SmolStr::new(reader.read_string()?);
                let value = decode_value(reader, physical_checkpoints)?;
                fields.insert(name, value);
            }
            let value = StructValue::from_untyped_parts(type_name, fields);
            let value = if physical_checkpoints {
                match reader.read_u8()? {
                    0 => value,
                    1 => {
                        let len = reader.read_u32()? as usize;
                        let checkpoint = reader.read_bytes(len)?;
                        value.with_physical_checkpoint(checkpoint).map_err(|_| {
                            RuntimeError::RetainCorruption(
                                "invalid physical struct checkpoint".into(),
                            )
                        })?
                    }
                    _ => {
                        return Err(RuntimeError::RetainCorruption(
                            "invalid physical struct checkpoint marker".into(),
                        ));
                    }
                }
            } else {
                value
            };
            Value::Struct(Arc::new(value))
        }
        x if x == ValueTag::Enum as u8 => {
            let type_name = SmolStr::new(reader.read_string()?);
            let variant_name = SmolStr::new(reader.read_string()?);
            let numeric_value = reader.read_i64()?;
            Value::Enum(Box::new(EnumValue::from_canonical_parts(
                type_name,
                variant_name,
                numeric_value,
            )))
        }
        x if x == ValueTag::Null as u8 => Value::Null,
        _ => {
            return Err(RuntimeError::RetainCorruption(
                "unknown retain value tag".into(),
            ))
        }
    };
    Ok(value)
}
