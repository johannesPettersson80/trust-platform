//! The `TRSTA001` installed-application container (specification 34 §5.1).
//!
//! A chip keeps its installed program and its deployment record in one container in the
//! `application` region. This module checks that container before exposing either payload, so a
//! damaged or half-written container reads as no installed program (DEPLOY-03) and never as a
//! program. [`admit`] then decodes the container's record and admits the deployment against the
//! device's descriptor before any state exists (specification 34 §5.1.1, DEPLOY-04). The bench
//! packs a container with `cargo xtask portability pack-project`, which calls [`encode`]; the
//! checks in [`decode`] are the only path to the payloads.

use alloc::vec::Vec;
use core::fmt;

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{DeploymentRecord, DeploymentRecordError, DeviceDescriptor};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::vm::PreparationLimits;

/// The magic that starts a container.
pub const MAGIC: [u8; 8] = *b"TRSTA001";
/// The entry kind of `program.stbc`.
pub const PROGRAM_KIND: u32 = 1;
/// The entry kind of the deployment record.
pub const RECORD_KIND: u32 = 2;

/// The magic, the header CRC-32, the total length and the entry count.
const HEADER_BYTES: usize = 20;
/// One entry: kind, payload offset, payload length and payload CRC-32.
const ENTRY_BYTES: usize = 16;
/// A version-1 container holds the program and the deployment record.
const ENTRY_COUNT: usize = 2;
/// The entries end here; a payload starts at or after it.
const HEADER_END: usize = HEADER_BYTES + ENTRY_COUNT * ENTRY_BYTES;
/// Offset of the header CRC-32.
const HEADER_CRC_OFFSET: usize = 8;
/// The header CRC-32 covers bytes [`HEADER_CRC_START`] to [`HEADER_END`].
const HEADER_CRC_START: usize = 12;
/// Offset of the total length.
const TOTAL_LENGTH_OFFSET: usize = 12;
/// Offset of the entry count.
const ENTRY_COUNT_OFFSET: usize = 16;

/// The two payloads of an installed application container, each a slice of the region.
pub struct Installed<'a> {
    /// The bytes of `program.stbc`.
    pub program: &'a [u8],
    /// The bytes of `deployment-record.bin`.
    pub record: &'a [u8],
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
            Self::NoProgram => formatter.write_str("no TRSTA001 container in the region"),
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
    let total = HEADER_END
        .checked_add(program.len())
        .and_then(|sum| sum.checked_add(record.len()))
        .ok_or(InstallationError::TooLarge)?;
    let total = u32::try_from(total).map_err(|_| InstallationError::TooLarge)?;
    let mut bytes = Vec::with_capacity(total as usize);
    bytes.extend_from_slice(&MAGIC);
    // The header CRC-32 is filled in once the entry table exists.
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&total.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(ENTRY_COUNT)
            .map_err(|_| InstallationError::TooLarge)?
            .to_le_bytes(),
    );
    for (kind, offset, payload) in [
        (PROGRAM_KIND, HEADER_END, program),
        (RECORD_KIND, HEADER_END + program.len(), record),
    ] {
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(offset)
                .map_err(|_| InstallationError::TooLarge)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .map_err(|_| InstallationError::TooLarge)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&crc32::checksum(payload).to_le_bytes());
    }
    debug_assert_eq!(bytes.len(), HEADER_END);
    let crc = crc32::checksum(&bytes[HEADER_CRC_START..HEADER_END]);
    bytes[HEADER_CRC_OFFSET..HEADER_CRC_START].copy_from_slice(&crc.to_le_bytes());
    bytes.extend_from_slice(program);
    bytes.extend_from_slice(record);
    Ok(bytes)
}

/// Check a region and return the program and record it holds.
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
    // The magic is checked first: a write that stopped before its last step leaves erased bytes
    // where the magic goes, so every partial write reads as no program (DEPLOY-03).
    if region.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
        return Err(InstallationError::NoProgram);
    }
    if region.len() < HEADER_END {
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
    if crc32::checksum(&region[HEADER_CRC_START..HEADER_END]) != word(HEADER_CRC_OFFSET) {
        return Err(InstallationError::HeaderChecksum);
    }
    let total = word(TOTAL_LENGTH_OFFSET) as usize;
    if total < HEADER_END || total > region.len() {
        return Err(InstallationError::TotalLength);
    }
    let count = word(ENTRY_COUNT_OFFSET);
    if count as usize != ENTRY_COUNT {
        return Err(InstallationError::EntryCount(count));
    }
    // Kind, payload offset, payload end and payload CRC-32 of each entry.
    let mut entries = [(0u32, 0usize, 0usize, 0u32); ENTRY_COUNT];
    let mut seen = [false; ENTRY_COUNT];
    for (index, entry) in entries.iter_mut().enumerate() {
        let base = HEADER_BYTES + index * ENTRY_BYTES;
        let kind = word(base);
        let slot = match kind {
            PROGRAM_KIND => 0,
            RECORD_KIND => 1,
            other => return Err(InstallationError::EntryKind(other)),
        };
        if seen[slot] {
            return Err(InstallationError::EntryKind(kind));
        }
        seen[slot] = true;
        let offset = word(base + 4) as usize;
        let length = word(base + 8) as usize;
        let crc = word(base + 12);
        let end = offset
            .checked_add(length)
            .ok_or(InstallationError::EntryRange)?;
        if offset < HEADER_END || end > total {
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
    let mut program = None;
    let mut record = None;
    for (kind, offset, end, _) in entries {
        let payload = &region[offset..end];
        if kind == PROGRAM_KIND {
            program = Some(payload);
        } else {
            record = Some(payload);
        }
    }
    Ok(Installed {
        program: program.ok_or(InstallationError::EntryKind(PROGRAM_KIND))?,
        record: record.ok_or(InstallationError::EntryKind(RECORD_KIND))?,
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

/// Admit the installed-application container in a chip's `application` region before any state
/// exists: [`decode`] the container, decode its deployment record and run the core's admission
/// with the device's descriptor, in that order (specification 34 §5.1, §5.1.1, DEPLOY-04).
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
    let record = DeploymentRecord::decode(installed.record).map_err(AdmissionError::Record)?;
    let limits = trust_runtime_core::deployment::admit(&record, installed.program, device)
        .map_err(AdmissionError::Deployment)?;
    Ok(Admitted {
        program: installed.program,
        record,
        limits,
    })
}

#[cfg(test)]
mod tests;
