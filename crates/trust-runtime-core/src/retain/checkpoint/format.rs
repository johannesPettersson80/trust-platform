//! The bytes of the checkpoint log: the slot header and the record (specification 34 §10.1).

use super::CheckpointIdentity;
use crate::crc32;

/// The magic that starts a slot header.
pub const SLOT_MAGIC: [u8; 4] = *b"TRCS";
/// The magic that starts a record.
pub const RECORD_MAGIC: [u8; 4] = *b"TRCK";
/// The commit marker, repeated to fill a record's commit field.
pub const COMMIT_MARKER: [u8; 4] = *b"DONE";
/// The format version of slot headers and records.
pub const FORMAT_VERSION: u16 = 1;
/// The length of a slot header in bytes.
pub const SLOT_HEADER_LEN: u32 = 16;
/// The length of a record header in bytes; the commit field follows it.
pub const RECORD_HEADER_LEN: u32 = 64;

/// The largest program unit the layout supports: one record header.
pub(super) const MAX_PROGRAM_UNIT: u32 = RECORD_HEADER_LEN;
/// The value of an erased byte.
pub(super) const ERASED: u8 = 0xFF;

/// A program unit's worth of bytes, the largest unit the layout supports.
pub(super) type Unit = [u8; MAX_PROGRAM_UNIT as usize];
/// The bytes of a slot header.
pub(super) type SlotHeaderBytes = [u8; SLOT_HEADER_LEN as usize];
/// The bytes of a record header.
pub(super) type RecordHeaderBytes = [u8; RECORD_HEADER_LEN as usize];

/// The slot header of a slot formatted with `epoch`.
pub(super) fn encode_slot_header(epoch: u32) -> SlotHeaderBytes {
    let mut bytes = [0; SLOT_HEADER_LEN as usize];
    bytes[0..4].copy_from_slice(&SLOT_MAGIC);
    bytes[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    // Bytes 6 and 7 are reserved and stay 0.
    bytes[8..12].copy_from_slice(&epoch.to_le_bytes());
    let crc = crc32::checksum(&bytes[..12]);
    bytes[12..16].copy_from_slice(&crc.to_le_bytes());
    bytes
}

/// The epoch of a valid slot header, or `None` when the header fails its check.
pub(super) fn decode_slot_header(bytes: &SlotHeaderBytes) -> Option<u32> {
    let valid = bytes[0..4] == SLOT_MAGIC
        && le_u16(bytes, 4) == FORMAT_VERSION
        && le_u16(bytes, 6) == 0
        && le_u32(bytes, 12) == crc32::checksum(&bytes[..12]);
    let epoch = le_u32(bytes, 8);
    (valid && epoch != 0).then_some(epoch)
}

/// The fields of a record header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecordHeader {
    /// The checkpoint sequence.
    pub(super) sequence: u64,
    /// The application and retain schema the checkpoint was written for.
    pub(super) identity: CheckpointIdentity,
    /// The payload length in bytes.
    pub(super) payload_len: u32,
    /// The CRC-32 of the payload.
    pub(super) payload_crc: u32,
}

impl RecordHeader {
    /// The header's bytes, ending with the CRC-32 of the bytes before it.
    pub(super) fn encode(&self) -> RecordHeaderBytes {
        let mut bytes = [0; RECORD_HEADER_LEN as usize];
        bytes[0..4].copy_from_slice(&RECORD_MAGIC);
        bytes[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        // Bytes 6 and 7 are the flags, 0.
        bytes[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..32].copy_from_slice(&self.identity.application);
        bytes[32..48].copy_from_slice(&self.identity.schema);
        bytes[48..52].copy_from_slice(&self.payload_len.to_le_bytes());
        bytes[52..56].copy_from_slice(&self.payload_crc.to_le_bytes());
        // Bytes 56 to 59 are reserved and stay 0.
        let crc = crc32::checksum(&bytes[..60]);
        bytes[60..64].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    /// The fields of a valid header, or `None` when it fails its check.
    pub(super) fn decode(bytes: &RecordHeaderBytes) -> Option<Self> {
        let valid = bytes[0..4] == RECORD_MAGIC
            && le_u16(bytes, 4) == FORMAT_VERSION
            && le_u16(bytes, 6) == 0
            && le_u32(bytes, 56) == 0
            && le_u32(bytes, 60) == crc32::checksum(&bytes[..60]);
        if !valid {
            return None;
        }
        let mut identity = CheckpointIdentity::default();
        identity.application.copy_from_slice(&bytes[16..32]);
        identity.schema.copy_from_slice(&bytes[32..48]);
        Some(Self {
            sequence: le_u64(bytes, 8),
            identity,
            payload_len: le_u32(bytes, 48),
            payload_crc: le_u32(bytes, 52),
        })
    }
}

/// The commit field of the largest length: the marker repeated. A record's field is its first
/// `commit_len` bytes.
pub(super) fn commit_field() -> Unit {
    let mut field = [0; MAX_PROGRAM_UNIT as usize];
    for word in field.as_chunks_mut::<{ COMMIT_MARKER.len() }>().0 {
        *word = COMMIT_MARKER;
    }
    field
}

/// Whether a commit field reads exactly as written. Its length is a multiple of four.
pub(super) fn is_committed(field: &[u8]) -> bool {
    field
        .chunks(COMMIT_MARKER.len())
        .all(|word| word == COMMIT_MARKER)
}

/// Whether every byte reads as erased.
pub(super) fn is_erased(bytes: &[u8]) -> bool {
    bytes.iter().all(|&byte| byte == ERASED)
}

/// `value` rounded up to a multiple of `unit`, a power of two; `value` + `unit` must fit.
pub(super) const fn align_up(value: u32, unit: u32) -> u32 {
    (value + (unit - 1)) & !(unit - 1)
}

fn le_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn le_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn le_u64(bytes: &[u8], at: usize) -> u64 {
    u64::from(le_u32(bytes, at)) | (u64::from(le_u32(bytes, at + 4)) << 32)
}
