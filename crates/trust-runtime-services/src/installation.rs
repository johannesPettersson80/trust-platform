//! The installed-application container of a chip's `application` region (specification 34 §5.1).
//!
//! A chip keeps its installed program and its deployment record in one container. Version
//! `TRSTA001` holds the two payloads. Version `TRSTA002` also holds an install sequence under the
//! header CRC-32, which the program store uses to order its two slots. This module checks a
//! container before exposing either payload, so a damaged or half-written container reads as no
//! installed program (DEPLOY-03) and never as a program. [`admit`] and [`admit_sequenced`] then
//! decode the container's record and admit the deployment against the device's descriptor before
//! any state exists (specification 34 §5.1.1, DEPLOY-04). The bench packs a `TRSTA001` container
//! with `cargo xtask portability pack-project`, which calls [`encode`]; the checks in [`decode`]
//! and [`decode_sequenced`] are the only paths to the payloads.

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{DeploymentRecord, DeploymentRecordError, DeviceDescriptor};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::vm::PreparationLimits;

/// The magic of version 1: the two payloads and no install sequence.
pub const MAGIC: [u8; 8] = *b"TRSTA001";
/// The magic of version 2: the two payloads and the install sequence, under the header CRC-32.
pub const SEQUENCED_MAGIC: [u8; 8] = *b"TRSTA002";
/// The entry kind of `program.stbc`.
pub const PROGRAM_KIND: u32 = 1;
/// The entry kind of the deployment record.
pub const RECORD_KIND: u32 = 2;
/// Offset of the install sequence in a version-2 container (specification 34 §5.1).
pub const SEQUENCE_OFFSET: usize = 16;
/// The bytes of a version-2 header, from the magic to the last entry. Its payloads start here.
pub const SEQUENCED_HEADER_BYTES: usize = Version::Sequenced.header_end();

/// Offset of the header CRC-32.
const HEADER_CRC_OFFSET: usize = 8;
/// The header CRC-32 covers the bytes from here to the end of the entry table.
const HEADER_CRC_START: usize = 12;
/// Offset of the total length.
const TOTAL_LENGTH_OFFSET: usize = 12;
/// One entry: kind, payload offset, payload length and payload CRC-32.
const ENTRY_BYTES: usize = 16;
/// A container holds the program and the deployment record.
const ENTRY_COUNT: usize = 2;
/// The bytes of one header word, such as the entry count.
const WORD_BYTES: usize = 4;

/// The two container versions. Version 2 has the install sequence between the total length and
/// the entry count; the rest of the layout is the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    /// `TRSTA001`.
    Plain,
    /// `TRSTA002`.
    Sequenced,
}

impl Version {
    const fn magic(self) -> [u8; 8] {
        match self {
            Self::Plain => MAGIC,
            Self::Sequenced => SEQUENCED_MAGIC,
        }
    }

    /// Offset of the entry count.
    const fn count_offset(self) -> usize {
        match self {
            Self::Plain => 16,
            Self::Sequenced => 24,
        }
    }

    /// Offset of the first entry.
    const fn entries_offset(self) -> usize {
        self.count_offset() + WORD_BYTES
    }

    /// The end of the header and the entry table; a payload starts at or after it.
    const fn header_end(self) -> usize {
        self.entries_offset() + ENTRY_COUNT * ENTRY_BYTES
    }
}

/// The two payloads of an installed application container, each a slice of the region.
pub struct Installed<'a> {
    /// The bytes of `program.stbc`.
    pub program: &'a [u8],
    /// The bytes of `deployment-record.bin`.
    pub record: &'a [u8],
}

/// The payloads of a `TRSTA002` container and the install sequence its header holds.
pub struct Sequenced<'a> {
    /// The install sequence. The committed container with the highest sequence is the newest.
    pub sequence: u64,
    /// The two payloads.
    pub installed: Installed<'a>,
}

/// Why a region holds no installed program a chip may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationError {
    /// The region does not start with the container magic; erased flash in particular.
    NoProgram,
    /// The region ends before the header and entry table it declares.
    Truncated,
    /// The header CRC-32 does not match bytes 12 to the end of the entry table.
    HeaderChecksum,
    /// The total length is below the header and entries or above the region.
    TotalLength,
    /// The entry count is not 2.
    EntryCount(u32),
    /// An entry's kind is neither 1 nor 2, or a kind appears twice.
    EntryKind(u32),
    /// An entry's range is inside the header or outside the total length.
    EntryRange,
    /// Two entries overlap.
    EntryOverlap,
    /// A payload's CRC-32 does not match; the number is the entry's kind.
    PayloadChecksum(u32),
    /// The container is longer than a `u32` can count; a writer refuses to build it.
    TooLarge,
}

impl fmt::Display for InstallationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProgram => {
                formatter.write_str("the region does not start with the container magic")
            }
            Self::Truncated => formatter.write_str("the container ends in its header"),
            Self::HeaderChecksum => {
                formatter.write_str("the container header CRC-32 does not match")
            }
            Self::TotalLength => formatter.write_str("the container length is outside the region"),
            Self::EntryCount(count) => {
                write!(
                    formatter,
                    "the container holds {count} entries, not {ENTRY_COUNT}"
                )
            }
            Self::EntryKind(kind) => write!(formatter, "entry kind {kind} is unknown or repeated"),
            Self::EntryRange => formatter.write_str("an entry's range is outside the container"),
            Self::EntryOverlap => formatter.write_str("two container entries overlap"),
            Self::PayloadChecksum(kind) => {
                write!(formatter, "the CRC-32 of entry kind {kind} does not match")
            }
            Self::TooLarge => formatter.write_str("the container is longer than a u32 can count"),
        }
    }
}

impl core::error::Error for InstallationError {}

/// Build the `TRSTA001` container of `program` and `record`.
///
/// The container holds `program` first and `record` second, exactly as [`decode`] reads it: the
/// header CRC-32 covers bytes 12 to the end of the entry table, and the total length is the whole
/// container (specification 34 §5.1). A writer to flash writes the two payloads first, then the
/// entry table and the header fields, and the magic last, so a write that stops early reads as no
/// installed program (DEPLOY-03); this function returns the bytes in container order.
///
/// # Errors
///
/// [`InstallationError::TooLarge`] when the container is longer than a `u32` can count.
pub fn encode(program: &[u8], record: &[u8]) -> Result<Vec<u8>, InstallationError> {
    write(Version::Plain, None, program, record)
}

/// Build the `TRSTA002` container of `program` and `record` with the install sequence `sequence`
/// (specification 34 §5.1). The sequence sits under the header CRC-32, at [`SEQUENCE_OFFSET`].
///
/// # Errors
///
/// [`InstallationError::TooLarge`] when the container is longer than a `u32` can count.
pub fn encode_sequenced(
    sequence: u64,
    program: &[u8],
    record: &[u8],
) -> Result<Vec<u8>, InstallationError> {
    write(Version::Sequenced, Some(sequence), program, record)
}

/// Writes a container of `version` in its layout's order: the magic, the header CRC-32 (filled in
/// last), the total length, the sequence of version 2, the entry count, the entry table, and then
/// the payloads.
fn write(
    version: Version,
    sequence: Option<u64>,
    program: &[u8],
    record: &[u8],
) -> Result<Vec<u8>, InstallationError> {
    let header_end = version.header_end();
    let total = header_end
        .checked_add(program.len())
        .and_then(|sum| sum.checked_add(record.len()))
        .ok_or(InstallationError::TooLarge)?;
    let total = u32::try_from(total).map_err(|_| InstallationError::TooLarge)?;
    let mut bytes = Vec::with_capacity(total as usize);
    bytes.extend_from_slice(&version.magic());
    // The header CRC-32 is filled in once the entry table exists.
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&total.to_le_bytes());
    if let Some(sequence) = sequence {
        bytes.extend_from_slice(&sequence.to_le_bytes());
    }
    let count = u32::try_from(ENTRY_COUNT).map_err(|_| InstallationError::TooLarge)?;
    bytes.extend_from_slice(&count.to_le_bytes());
    for (kind, offset, payload) in [
        (PROGRAM_KIND, header_end, program),
        (RECORD_KIND, header_end + program.len(), record),
    ] {
        let offset = u32::try_from(offset).map_err(|_| InstallationError::TooLarge)?;
        let length = u32::try_from(payload.len()).map_err(|_| InstallationError::TooLarge)?;
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&crc32::checksum(payload).to_le_bytes());
    }
    debug_assert_eq!(bytes.len(), header_end);
    let crc = crc32::checksum(&bytes[HEADER_CRC_START..header_end]);
    bytes[HEADER_CRC_OFFSET..HEADER_CRC_START].copy_from_slice(&crc.to_le_bytes());
    bytes.extend_from_slice(program);
    bytes.extend_from_slice(record);
    Ok(bytes)
}

/// The byte ranges of the two payloads of a container that passed its checks.
struct Payloads {
    program: Range<usize>,
    record: Range<usize>,
}

/// Checks a container of `version` in the order of specification 34 §5.1 and returns where its
/// payloads lie. Nothing is returned before every check has passed.
fn check(region: &[u8], version: Version) -> Result<Payloads, InstallationError> {
    // The magic is checked first: a write that stopped before its last step leaves erased bytes
    // where the magic goes, so every partial write reads as no program (DEPLOY-03).
    let magic = version.magic();
    if region.get(..magic.len()) != Some(magic.as_slice()) {
        return Err(InstallationError::NoProgram);
    }
    let header_end = version.header_end();
    if region.len() < header_end {
        return Err(InstallationError::Truncated);
    }
    let word = |offset: usize| {
        u32::from_le_bytes([
            region[offset],
            region[offset + 1],
            region[offset + 2],
            region[offset + 3],
        ])
    };
    if crc32::checksum(&region[HEADER_CRC_START..header_end]) != word(HEADER_CRC_OFFSET) {
        return Err(InstallationError::HeaderChecksum);
    }
    let total = word(TOTAL_LENGTH_OFFSET) as usize;
    if total < header_end || total > region.len() {
        return Err(InstallationError::TotalLength);
    }
    let count = word(version.count_offset());
    if count as usize != ENTRY_COUNT {
        return Err(InstallationError::EntryCount(count));
    }
    // Kind, payload offset, payload end and payload CRC-32 of each entry.
    let mut entries = [(0u32, 0usize, 0usize, 0u32); ENTRY_COUNT];
    let mut seen = [false; ENTRY_COUNT];
    for (index, entry) in entries.iter_mut().enumerate() {
        let base = version.entries_offset() + index * ENTRY_BYTES;
        let kind = word(base);
        let position = match kind {
            PROGRAM_KIND => 0,
            RECORD_KIND => 1,
            other => return Err(InstallationError::EntryKind(other)),
        };
        if seen[position] {
            return Err(InstallationError::EntryKind(kind));
        }
        seen[position] = true;
        let offset = word(base + 4) as usize;
        let length = word(base + 8) as usize;
        let crc = word(base + 12);
        let end = offset
            .checked_add(length)
            .ok_or(InstallationError::EntryRange)?;
        if offset < header_end || end > total {
            return Err(InstallationError::EntryRange);
        }
        *entry = (kind, offset, end, crc);
    }
    if entries[0].1 < entries[1].2 && entries[1].1 < entries[0].2 {
        return Err(InstallationError::EntryOverlap);
    }
    for (kind, offset, end, crc) in entries {
        if crc32::checksum(&region[offset..end]) != crc {
            return Err(InstallationError::PayloadChecksum(kind));
        }
    }
    // Each kind appears once, so the two entries are the program and the record in some order.
    let [first, second] = entries;
    let (program, record) = if first.0 == PROGRAM_KIND {
        (first, second)
    } else {
        (second, first)
    };
    Ok(Payloads {
        program: program.1..program.2,
        record: record.1..record.2,
    })
}

/// Check a `TRSTA001` region and return the program and record it holds.
///
/// Nothing is returned before every check has passed: the magic, the header CRC-32, the total
/// length against the region, the entry count, each kind once, every entry's range and overlap,
/// and every payload's CRC-32 (specification 34 §5.1).
///
/// # Errors
///
/// [`InstallationError::NoProgram`] when the region is erased or does not start with the magic;
/// otherwise the [`InstallationError`] naming the first check that failed.
pub fn decode(region: &[u8]) -> Result<Installed<'_>, InstallationError> {
    let Payloads { program, record } = check(region, Version::Plain)?;
    Ok(Installed {
        program: &region[program],
        record: &region[record],
    })
}

/// Check a `TRSTA002` region and return its payloads and its install sequence, with the same
/// checks as [`decode`] (specification 34 §5.1). The sequence sits under the header CRC-32, so a
/// sequence other than the one written fails the header check.
///
/// # Errors
///
/// As [`decode`], with [`InstallationError::NoProgram`] when the region does not start with the
/// `TRSTA002` magic.
pub fn decode_sequenced(region: &[u8]) -> Result<Sequenced<'_>, InstallationError> {
    let Payloads { program, record } = check(region, Version::Sequenced)?;
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&region[SEQUENCE_OFFSET..SEQUENCE_OFFSET + 8]);
    Ok(Sequenced {
        sequence: u64::from_le_bytes(sequence),
        installed: Installed {
            program: &region[program],
            record: &region[record],
        },
    })
}

/// Write `sequence` into the header of the `TRSTA002` container `container` and recompute its
/// header CRC-32. The device gives each commit its own sequence, so the sequence a transferred
/// container carries is replaced before the commit (specification 34 §5.1).
///
/// # Errors
///
/// [`InstallationError::NoProgram`] when `container` does not start with the `TRSTA002` magic, and
/// [`InstallationError::Truncated`] when it ends inside its header.
pub fn set_sequence(container: &mut [u8], sequence: u64) -> Result<(), InstallationError> {
    if container.get(..SEQUENCED_MAGIC.len()) != Some(SEQUENCED_MAGIC.as_slice()) {
        return Err(InstallationError::NoProgram);
    }
    let Some(header) = container.get_mut(..SEQUENCED_HEADER_BYTES) else {
        return Err(InstallationError::Truncated);
    };
    header[SEQUENCE_OFFSET..SEQUENCE_OFFSET + 8].copy_from_slice(&sequence.to_le_bytes());
    let crc = crc32::checksum(&header[HEADER_CRC_START..]);
    header[HEADER_CRC_OFFSET..HEADER_CRC_START].copy_from_slice(&crc.to_le_bytes());
    Ok(())
}

/// The header of a committed `TRSTA002` container: its total length and its install sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequencedHeader {
    /// The container's total length in bytes.
    pub total_len: u32,
    /// The install sequence.
    pub sequence: u64,
}

/// Read the header of a `TRSTA002` container from the first [`SEQUENCED_HEADER_BYTES`] of a slot
/// of `slot_len` bytes. It returns the header only when the magic reads `TRSTA002`, the header
/// CRC-32 matches and the total length lies between the header and the end of the slot
/// (specification 34 §5.1). Any other slot holds no committed container, and its bytes are not
/// trusted (DEPLOY-03).
#[must_use]
pub fn committed_header(header: &[u8], slot_len: u32) -> Option<SequencedHeader> {
    let header = header.get(..SEQUENCED_HEADER_BYTES)?;
    if header.get(..SEQUENCED_MAGIC.len()) != Some(SEQUENCED_MAGIC.as_slice()) {
        return None;
    }
    let word = |offset: usize| {
        u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    };
    if crc32::checksum(&header[HEADER_CRC_START..]) != word(HEADER_CRC_OFFSET) {
        return None;
    }
    let total_len = word(TOTAL_LENGTH_OFFSET);
    if (total_len as usize) < SEQUENCED_HEADER_BYTES || total_len > slot_len {
        return None;
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&header[SEQUENCE_OFFSET..SEQUENCE_OFFSET + 8]);
    Some(SequencedHeader {
        total_len,
        sequence: u64::from_le_bytes(sequence),
    })
}

/// An installed application a chip admitted from its `application` region: the program, its
/// decoded record and the preparation limits the device granted (specification 34 §5.1, §5.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Admitted<'a> {
    /// The bytes of `program.stbc`.
    pub program: &'a [u8],
    /// The deployment record the container holds.
    pub record: DeploymentRecord,
    /// The preparation limits admission granted; prepare the program with exactly these.
    pub limits: PreparationLimits,
}

/// A `TRSTA002` container admitted from a region: the install sequence it holds and what it
/// admitted.
#[derive(Debug, Clone, PartialEq)]
pub struct SequencedAdmitted<'a> {
    /// The install sequence the header holds.
    pub sequence: u64,
    /// The admitted program, record and limits.
    pub admitted: Admitted<'a>,
}

/// Why a chip admits no installed application (specification 34 §5.1, §5.1.1, DEPLOY-04). Each
/// variant keeps the error of the step that refused, so its reason and numbers are not lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// The region holds no whole container: [`InstallationError::NoProgram`], an erased region in
    /// particular, and the other container errors (specification 34 §5.1).
    Container(InstallationError),
    /// The container's record is not one this runtime reads; the same refusal the core reports as
    /// `RuntimeError::DeploymentRecordInvalid` (specification 34 §5.1.1).
    Record(DeploymentRecordError),
    /// Admission refused the deployment: `RuntimeError::DeploymentRefused` with its numbers, or
    /// `RuntimeError::ImportUnsupported` (specification 34 §5.1.1, DEPLOY-04).
    Deployment(RuntimeError),
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(formatter, "no installed application: {error}"),
            Self::Record(error) => write!(formatter, "invalid deployment record: {error}"),
            Self::Deployment(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl core::error::Error for AdmissionError {}

/// Admit the `TRSTA001` container in a chip's `application` region before any state exists:
/// [`decode`] the container, decode its deployment record and run the core's admission with the
/// device's descriptor, in that order (specification 34 §5.1, §5.1.1, DEPLOY-04).
///
/// Nothing is constructed on any path: the checks run on the container's bytes alone, and an
/// admitted container returns the program, its decoded record and the limits to prepare it with.
///
/// # Errors
///
/// [`AdmissionError::Container`] when the region holds no whole container,
/// [`AdmissionError::Record`] when its record is not one this runtime reads, and
/// [`AdmissionError::Deployment`] when admission refuses the deployment; each keeps the error and
/// the numbers of the step that refused.
pub fn admit<'a>(
    region: &'a [u8],
    device: &DeviceDescriptor,
) -> Result<Admitted<'a>, AdmissionError> {
    let installed = decode(region).map_err(AdmissionError::Container)?;
    let (record, limits) = admit_payloads(installed.program, installed.record, device)?;
    Ok(Admitted {
        program: installed.program,
        record,
        limits,
    })
}

/// Admit the `TRSTA002` container in a chip's `application` region, as [`admit`] does for
/// version 1, and return the install sequence it holds with the admitted program (specification
/// 34 §5.1, §5.1.1, DEPLOY-04).
///
/// # Errors
///
/// As [`admit`].
pub fn admit_sequenced<'a>(
    region: &'a [u8],
    device: &DeviceDescriptor,
) -> Result<SequencedAdmitted<'a>, AdmissionError> {
    let Sequenced {
        sequence,
        installed,
    } = decode_sequenced(region).map_err(AdmissionError::Container)?;
    let (record, limits) = admit_payloads(installed.program, installed.record, device)?;
    Ok(SequencedAdmitted {
        sequence,
        admitted: Admitted {
            program: installed.program,
            record,
            limits,
        },
    })
}

/// The admission both versions share: the record is decoded, and the core admits the program with
/// it against the device.
fn admit_payloads(
    program: &[u8],
    record: &[u8],
    device: &DeviceDescriptor,
) -> Result<(DeploymentRecord, PreparationLimits), AdmissionError> {
    let record = DeploymentRecord::decode(record).map_err(AdmissionError::Record)?;
    let limits = trust_runtime_core::deployment::admit(&record, program, device)
        .map_err(AdmissionError::Deployment)?;
    Ok((record, limits))
}

#[cfg(test)]
mod tests;
