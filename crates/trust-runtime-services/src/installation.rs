//! The installed-application container of a chip's `application` region (specification 34 §5.1).
//!
//! A chip keeps its installed program and its deployment record in one container. Version
//! `TRSTA002` holds the two payloads and an install sequence under the header CRC-32, which the
//! program store uses to order its two slots. It is the only version that the program store reads,
//! that any boot reads and that a tool writes: [`encode_sequenced`] is the one writer, and
//! `cargo xtask portability pack-project` calls it with [`FIRST_SEQUENCE`]. Version `TRSTA001` is
//! the same container without the sequence. It is not committed (specification 34 §5.1): only the
//! tests of this module build and read one, to prove that the store refuses it. This module checks
//! a container before exposing either payload, so a damaged or half-written container reads as no
//! installed program (DEPLOY-03) and never as a program. [`admit_sequenced`] then decodes the
//! container's record and admits the deployment against the device's descriptor before any state
//! exists (specification 34 §5.1.1, DEPLOY-04); the checks in [`decode_sequenced`] are the only
//! path to the payloads. [`sequenced_layout`] reads only the header of a container, which is how
//! `program.active` finds the record of a container too large to hold in memory.
//!
//! The program of a container is `program.stbc` (entry kind 1) or a compiled program image (entry
//! kind 3, specification 44 §10.1), whose first byte the writer puts at a multiple of 64 bytes from
//! the container's start, so that an image a device reads in place keeps the alignment of its
//! sections (specification 44 §5.3). A container holds one program and its record; its record names
//! the program's kind by its version, and admission refuses a container whose kind and record differ.

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    AdmissionRefusal, DeploymentRecord, DeploymentRecordError, DeviceDescriptor,
};
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
/// The entry kind of a compiled program image (specification 44 §10.1).
pub const IMAGE_KIND: u32 = 3;
/// A compiled program image starts at a multiple of this many bytes from the container's start
/// (specification 44 §5.3: `CODE` lies at a 64-byte boundary of the image).
pub const IMAGE_ALIGNMENT: usize = 64;
/// Offset of the install sequence in a version-2 container (specification 34 §5.1).
pub const SEQUENCE_OFFSET: usize = 16;
/// The install sequence of the first commit into an empty store. The program store gives it to that
/// container, and the bench packer writes it, so that a packed container written into slot A is a
/// committed container with the bytes of the first commit (specification 34 §5.1, "A first flash").
/// Each later commit takes one more than the newest committed sequence.
pub const FIRST_SEQUENCE: u64 = 1;
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
/// the entry count; the rest of the layout is the same. Version 1 exists in the tests only: no
/// boot and no tool reads or writes it (specification 34 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    /// `TRSTA001`.
    #[cfg(test)]
    Plain,
    /// `TRSTA002`.
    Sequenced,
}

impl Version {
    const fn magic(self) -> [u8; 8] {
        match self {
            #[cfg(test)]
            Self::Plain => MAGIC,
            Self::Sequenced => SEQUENCED_MAGIC,
        }
    }

    /// Offset of the entry count.
    const fn count_offset(self) -> usize {
        match self {
            #[cfg(test)]
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

/// What the program of a container is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramFormat {
    /// `program.stbc`, entry kind [`PROGRAM_KIND`].
    Bytecode,
    /// A compiled program image, entry kind [`IMAGE_KIND`] (specification 44 §10.1).
    Image,
}

impl ProgramFormat {
    /// The entry kind of a program of this format.
    #[must_use]
    pub const fn kind(self) -> u32 {
        match self {
            Self::Bytecode => PROGRAM_KIND,
            Self::Image => IMAGE_KIND,
        }
    }

    /// The format of a program whose entry kind is `kind`, `None` for any other kind.
    const fn of_kind(kind: u32) -> Option<Self> {
        match kind {
            PROGRAM_KIND => Some(Self::Bytecode),
            IMAGE_KIND => Some(Self::Image),
            _ => None,
        }
    }

    /// Where the program of this format starts in a container whose header ends at `header_end`.
    const fn program_offset(self, header_end: usize) -> usize {
        match self {
            Self::Bytecode => header_end,
            Self::Image => header_end.next_multiple_of(IMAGE_ALIGNMENT),
        }
    }
}

/// The two payloads of an installed application container, each a slice of the region.
pub struct Installed<'a> {
    /// What the program is.
    pub format: ProgramFormat,
    /// The bytes of the program: `program.stbc` or the image.
    pub program: &'a [u8],
    /// The bytes of `deployment-record.bin`.
    pub record: &'a [u8],
}

/// A container as a reader finds it: the bytes that hold its header, and the region from its start in
/// which its payloads lie at their offsets. A container in memory is both at once
/// ([`ContainerBytes::whole`]). One that a chip wrote into its program store has its header in the
/// transfer session and its payloads in the slot, whose first bytes are erased until the install
/// programs the header (specification 34 §5.1); a reader reads no byte of the header from the region.
#[derive(Debug, Clone, Copy)]
pub struct ContainerBytes<'a> {
    /// The container's first bytes: at least its header and entry table.
    pub header: &'a [u8],
    /// The container's bytes from its start, of which a reader reads only the payloads.
    pub region: &'a [u8],
}

impl<'a> ContainerBytes<'a> {
    /// The container `container`, all of whose bytes are in memory.
    #[must_use]
    pub const fn whole(container: &'a [u8]) -> Self {
        Self {
            header: container,
            region: container,
        }
    }
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
    /// An entry's kind is none of 1, 2 and 3, a kind appears twice, or the container holds two
    /// programs.
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

/// Build the `TRSTA001` container of `program` and `record`, as the tests of the version-1 reader
/// and of the program store's refusal of version 1 need it. No tool writes version 1: the writer of
/// every container is [`encode_sequenced`] (specification 34 §5.1).
///
/// The container holds `program` first and `record` second, exactly as [`decode`] reads it: the
/// header CRC-32 covers bytes 12 to the end of the entry table, and the total length is the whole
/// container.
///
/// # Errors
///
/// [`InstallationError::TooLarge`] when the container is longer than a `u32` can count.
#[cfg(test)]
pub fn encode(program: &[u8], record: &[u8]) -> Result<Vec<u8>, InstallationError> {
    write(
        Version::Plain,
        None,
        ProgramFormat::Bytecode,
        program,
        record,
    )
}

/// Build the `TRSTA002` container of `program` and `record` with the install sequence `sequence`
/// (specification 34 §5.1). The sequence sits under the header CRC-32, at [`SEQUENCE_OFFSET`].
///
/// This is the one writer of the container: the bench packer calls it with [`FIRST_SEQUENCE`], and
/// a client that uploads a program for `program.install` calls it with any sequence, since the
/// device replaces the sequence with its own ([`set_sequence`]). The bytes are in container order; a
/// writer to flash writes the two payloads first, then the entry table and the header fields, and
/// the magic last, so a write that stops early reads as no installed program (DEPLOY-03).
///
/// # Errors
///
/// [`InstallationError::TooLarge`] when the container is longer than a `u32` can count.
pub fn encode_sequenced(
    sequence: u64,
    program: &[u8],
    record: &[u8],
) -> Result<Vec<u8>, InstallationError> {
    write(
        Version::Sequenced,
        Some(sequence),
        ProgramFormat::Bytecode,
        program,
        record,
    )
}

/// Build the `TRSTA002` container of the compiled program image `image` and `record` with the
/// install sequence `sequence` (specification 44 §10.1): [`encode_sequenced`]'s container with the
/// entry kind [`IMAGE_KIND`] and the image at the first multiple of [`IMAGE_ALIGNMENT`] after the
/// header, zeros between.
///
/// # Errors
///
/// [`InstallationError::TooLarge`] when the container is longer than a `u32` can count.
pub fn encode_image(
    sequence: u64,
    image: &[u8],
    record: &[u8],
) -> Result<Vec<u8>, InstallationError> {
    write(
        Version::Sequenced,
        Some(sequence),
        ProgramFormat::Image,
        image,
        record,
    )
}

/// Writes a container of `version` in its layout's order: the magic, the header CRC-32 (filled in
/// last), the total length, the sequence of version 2, the entry count, the entry table, and then
/// the payloads, the program of `format` where that format starts.
fn write(
    version: Version,
    sequence: Option<u64>,
    format: ProgramFormat,
    program: &[u8],
    record: &[u8],
) -> Result<Vec<u8>, InstallationError> {
    let header_end = version.header_end();
    let program_offset = format.program_offset(header_end);
    let total = program_offset
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
        (format.kind(), program_offset, program),
        (RECORD_KIND, program_offset + program.len(), record),
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
    bytes.resize(program_offset, 0);
    bytes.extend_from_slice(program);
    bytes.extend_from_slice(record);
    Ok(bytes)
}

/// The byte ranges of the two payloads of a container that passed its checks.
struct Payloads {
    format: ProgramFormat,
    program: Range<usize>,
    record: Range<usize>,
}

/// One entry of a container's table: the kind of payload, where it lies and the CRC-32 it must have.
#[derive(Debug, Clone, Copy)]
struct Entry {
    kind: u32,
    offset: usize,
    end: usize,
    crc: u32,
}

/// Checks the header of a container of `version` in the order of specification 34 §5.1, up to and
/// including its entry table, and returns the entries in the table's order. `header` holds at
/// least the header and the entry table, and `limit` is the bytes the container may take: the
/// region's length, or the total length of a container whose payloads are not in memory. The
/// payloads' CRC-32 are not checked here.
fn entries(
    header: &[u8],
    version: Version,
    limit: usize,
) -> Result<[Entry; ENTRY_COUNT], InstallationError> {
    // The magic is checked first: a write that stopped before its last step leaves erased bytes
    // where the magic goes, so every partial write reads as no program (DEPLOY-03).
    let magic = version.magic();
    if header.get(..magic.len()) != Some(magic.as_slice()) {
        return Err(InstallationError::NoProgram);
    }
    let header_end = version.header_end();
    if header.len() < header_end {
        return Err(InstallationError::Truncated);
    }
    let word = |offset: usize| {
        u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    };
    if crc32::checksum(&header[HEADER_CRC_START..header_end]) != word(HEADER_CRC_OFFSET) {
        return Err(InstallationError::HeaderChecksum);
    }
    let total = word(TOTAL_LENGTH_OFFSET) as usize;
    if total < header_end || total > limit {
        return Err(InstallationError::TotalLength);
    }
    let count = word(version.count_offset());
    if count as usize != ENTRY_COUNT {
        return Err(InstallationError::EntryCount(count));
    }
    let mut table = [Entry {
        kind: 0,
        offset: 0,
        end: 0,
        crc: 0,
    }; ENTRY_COUNT];
    let mut seen = [false; ENTRY_COUNT];
    for (index, entry) in table.iter_mut().enumerate() {
        let base = version.entries_offset() + index * ENTRY_BYTES;
        let kind = word(base);
        let position = match kind {
            RECORD_KIND => 1,
            other if ProgramFormat::of_kind(other).is_some() => 0,
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
        *entry = Entry {
            kind,
            offset,
            end,
            crc,
        };
    }
    if table[0].offset < table[1].end && table[1].offset < table[0].end {
        return Err(InstallationError::EntryOverlap);
    }
    Ok(table)
}

/// Checks a container of `version` in the order of specification 34 §5.1 and returns where its
/// payloads lie. Nothing is returned before every check has passed.
fn check(container: ContainerBytes<'_>, version: Version) -> Result<Payloads, InstallationError> {
    let table = entries(container.header, version, container.region.len())?;
    for entry in table {
        if crc32::checksum(&container.region[entry.offset..entry.end]) != entry.crc {
            return Err(InstallationError::PayloadChecksum(entry.kind));
        }
    }
    let (program, record) = program_and_record(table);
    Ok(Payloads {
        format: ProgramFormat::of_kind(program.kind).unwrap_or(ProgramFormat::Bytecode),
        program: program.offset..program.end,
        record: record.offset..record.end,
    })
}

/// The program's entry and the record's: the table holds one program and one record in some order,
/// as [`entries`] checked.
fn program_and_record([first, second]: [Entry; ENTRY_COUNT]) -> (Entry, Entry) {
    if first.kind == RECORD_KIND {
        (second, first)
    } else {
        (first, second)
    }
}

/// Check a `TRSTA001` region and return the program and record it holds. Only the tests call this: no
/// boot reads `TRSTA001` (see the module documentation).
///
/// Nothing is returned before every check has passed: the magic, the header CRC-32, the total
/// length against the region, the entry count, each kind once, every entry's range and overlap,
/// and every payload's CRC-32 (specification 34 §5.1).
///
/// # Errors
///
/// [`InstallationError::NoProgram`] when the region is erased or does not start with the magic;
/// otherwise the [`InstallationError`] naming the first check that failed.
#[cfg(test)]
pub fn decode(region: &[u8]) -> Result<Installed<'_>, InstallationError> {
    let Payloads {
        format,
        program,
        record,
    } = check(ContainerBytes::whole(region), Version::Plain)?;
    Ok(Installed {
        format,
        program: &region[program],
        record: &region[record],
    })
}

/// Check a `TRSTA002` region and return its payloads and its install sequence. Nothing is returned
/// before every check has passed: the magic, the header CRC-32, the total length against the region,
/// the entry count, each kind once, every entry's range and overlap, and every payload's CRC-32
/// (specification 34 §5.1). The sequence sits under the header CRC-32, so a sequence other than the
/// one written fails the header check.
///
/// # Errors
///
/// [`InstallationError::NoProgram`] when the region is erased or does not start with the `TRSTA002`
/// magic; otherwise the [`InstallationError`] naming the first check that failed.
pub fn decode_sequenced(region: &[u8]) -> Result<Sequenced<'_>, InstallationError> {
    decode_sequenced_bytes(ContainerBytes::whole(region))
}

/// [`decode_sequenced`] of a container whose header and payloads may lie apart: the header is read
/// from `container.header` and the payloads from `container.region` (specification 34 §5.1, an
/// upload written into the store). The checks and their order are those of [`decode_sequenced`].
///
/// # Errors
///
/// Those of [`decode_sequenced`].
pub fn decode_sequenced_bytes(
    container: ContainerBytes<'_>,
) -> Result<Sequenced<'_>, InstallationError> {
    let Payloads {
        format,
        program,
        record,
    } = check(container, Version::Sequenced)?;
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&container.header[SEQUENCE_OFFSET..SEQUENCE_OFFSET + 8]);
    Ok(Sequenced {
        sequence: u64::from_le_bytes(sequence),
        installed: Installed {
            format,
            program: &container.region[program],
            record: &container.region[record],
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

/// Where the record of a committed `TRSTA002` container lies, and how long its program is, read from
/// the container's header alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencedLayout {
    /// The program's length in bytes.
    pub program_bytes: usize,
    /// The record's range in the container, in bytes from its start.
    pub record: Range<usize>,
    /// The CRC-32 that the record's bytes must have.
    pub record_crc: u32,
}

/// Reads the layout of the committed `TRSTA002` container whose header is `header` and whose total
/// length is `total_len`, without a byte of its payloads (specification 34 §5.1, `program.active`).
/// `header` holds at least the first [`SEQUENCED_HEADER_BYTES`] of the container.
///
/// The checks are those of [`decode_sequenced`] up to and including the entry table, in the same
/// order: the magic, the header CRC-32, the total length (against `total_len`, which the caller took
/// from [`committed_header`]), the entry count, each kind once, every entry's range and that no two
/// entries overlap. The payloads' CRC-32 are not checked: the caller reads the record and checks its
/// CRC-32 against [`SequencedLayout::record_crc`].
///
/// # Errors
///
/// The [`InstallationError`] of the first check that failed.
pub fn sequenced_layout(
    header: &[u8],
    total_len: u32,
) -> Result<SequencedLayout, InstallationError> {
    let table = entries(header, Version::Sequenced, total_len as usize)?;
    let (program, record) = program_and_record(table);
    Ok(SequencedLayout {
        program_bytes: program.end - program.offset,
        record: record.offset..record.end,
        record_crc: record.crc,
    })
}

/// An installed application a chip admitted from its `application` region: the program, its
/// decoded record and the preparation limits the device granted (specification 34 §5.1, §5.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Admitted<'a> {
    /// What the program is.
    pub format: ProgramFormat,
    /// The bytes of the program: `program.stbc` or the image.
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
/// device's descriptor, in that order (specification 34 §5.1, §5.1.1, DEPLOY-04). Only the tests call
/// this: every boot admits with [`admit_sequenced`].
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
#[cfg(test)]
pub fn admit<'a>(
    region: &'a [u8],
    device: &DeviceDescriptor,
) -> Result<Admitted<'a>, AdmissionError> {
    let installed = decode(region).map_err(AdmissionError::Container)?;
    let (record, limits) = admit_payloads(&installed, device)?;
    Ok(Admitted {
        format: installed.format,
        program: installed.program,
        record,
        limits,
    })
}

/// Admit the `TRSTA002` container in a chip's `application` region before any state exists: decode
/// the container ([`decode_sequenced`]), decode its deployment record and run the core's admission
/// with the device's descriptor, in that order, and return the install sequence it holds with the
/// admitted program (specification 34 §5.1, §5.1.1, DEPLOY-04). Nothing is constructed on any path:
/// the checks run on the container's bytes alone.
///
/// # Errors
///
/// [`AdmissionError::Container`] when the region holds no whole container,
/// [`AdmissionError::Record`] when its record is not one this runtime reads, and
/// [`AdmissionError::Deployment`] when admission refuses the deployment; each keeps the error and
/// the numbers of the step that refused.
pub fn admit_sequenced<'a>(
    region: &'a [u8],
    device: &DeviceDescriptor,
) -> Result<SequencedAdmitted<'a>, AdmissionError> {
    admit_sequenced_bytes(ContainerBytes::whole(region), device)
}

/// [`admit_sequenced`] of a container whose header and payloads may lie apart, as
/// [`decode_sequenced_bytes`] reads it: an upload that a chip wrote into its program store, which the
/// install admits before it programs the header (specification 34 §5.1, §11.2).
///
/// # Errors
///
/// Those of [`admit_sequenced`].
pub fn admit_sequenced_bytes<'a>(
    container: ContainerBytes<'a>,
    device: &DeviceDescriptor,
) -> Result<SequencedAdmitted<'a>, AdmissionError> {
    let Sequenced {
        sequence,
        installed,
    } = decode_sequenced_bytes(container).map_err(AdmissionError::Container)?;
    let (record, limits) = admit_payloads(&installed, device)?;
    Ok(SequencedAdmitted {
        sequence,
        admitted: Admitted {
            format: installed.format,
            program: installed.program,
            record,
            limits,
        },
    })
}

/// The admission both versions share: the record is decoded, a record of another kind of program
/// than the container's is refused as a record of another program, and the core admits the program
/// with its record against the device.
fn admit_payloads(
    installed: &Installed<'_>,
    device: &DeviceDescriptor,
) -> Result<(DeploymentRecord, PreparationLimits), AdmissionError> {
    let record = DeploymentRecord::decode(installed.record).map_err(AdmissionError::Record)?;
    let names_image = record.application.bytecode_version == BytecodeVersion::NATIVE_IMAGE;
    if names_image != (installed.format == ProgramFormat::Image) {
        return Err(AdmissionError::Deployment(RuntimeError::DeploymentRefused(
            AdmissionRefusal::ProgramMismatch,
        )));
    }
    let limits = trust_runtime_core::deployment::admit(&record, installed.program, device)
        .map_err(AdmissionError::Deployment)?;
    Ok((record, limits))
}

#[cfg(test)]
mod tests;
