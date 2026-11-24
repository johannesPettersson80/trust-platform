//! Reads a `.trusttrace` file (specification 48 Appendix C).

use alloc::string::String;
use alloc::vec::Vec;

use super::{
    section, Activation, Change, ChangeKind, ChangeSource, Cycle, CycleFault, ImageTables, IoArea,
    Located, Meta, StartState, Trace, TraceEnd, TraceError, TraceOrigin, FORMAT_VERSION,
    HEADER_BYTES, MAGIC, SECTION_ENTRY_BYTES,
};
use crate::crc32;

/// A trace read from its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The trace, its cycles up to the first damaged one.
    pub trace: Trace,
    /// The index of the first damaged cycle, at which the trace ends (Appendix C, C.2).
    pub damaged_cycle: Option<usize>,
}

/// Reads a `.trusttrace` file: its header, its section table, every required section, and its cycles
/// until the first damaged one (Appendix C, C.2).
///
/// # Errors
///
/// A file that is not a trace of format version 1, or whose header, section table or a required
/// section is cut short, damaged or malformed.
pub fn decode(bytes: &[u8]) -> Result<Decoded, TraceError> {
    let entries = section_table(bytes)?;
    let required = |id: u32| -> Result<&[u8], TraceError> {
        let mut found = entries.iter().filter(|entry| entry.id == id);
        let (Some(entry), None) = (found.next(), found.next()) else {
            let count = entries.iter().filter(|entry| entry.id == id).count();
            return Err(TraceError::SectionCount {
                name: name(id),
                count,
            });
        };
        let content = &bytes[entry.range()];
        if crc32::checksum(content) != entry.crc {
            return Err(TraceError::SectionDamaged(name(id)));
        }
        Ok(content)
    };
    let strings = required(section::STRS)?;
    if strings.first() != Some(&0) {
        return Err(malformed(
            section::STRS,
            "it does not start with the empty string",
        ));
    }
    let meta = read_meta(required(section::META)?, strings)?;
    let located = read_syms(required(section::SYMS)?, strings)?;
    let start = read_stat(required(section::STAT)?)?;
    let image = ImageTables {
        areas: required(section::IMAGE_AREAS)?.to_vec(),
        tasks: required(section::IMAGE_TASKS)?.to_vec(),
        iobind: required(section::IMAGE_IOBIND)?.to_vec(),
        symbols: required(section::IMAGE_SYMBOLS)?.to_vec(),
        types: required(section::IMAGE_TYPES)?.to_vec(),
        strings: required(section::IMAGE_STRINGS)?.to_vec(),
    };
    let mut cycles = Vec::new();
    let mut damaged_cycle = None;
    for (index, entry) in entries
        .iter()
        .filter(|entry| entry.id == section::CYCL)
        .enumerate()
    {
        let content = &bytes[entry.range()];
        let cycle = (crc32::checksum(content) == entry.crc)
            .then(|| read_cycle(content).ok())
            .flatten();
        let Some(cycle) = cycle else {
            damaged_cycle = Some(index);
            break;
        };
        cycles.push(cycle);
    }
    Ok(Decoded {
        trace: Trace {
            meta,
            located,
            start,
            image,
            cycles,
        },
        damaged_cycle,
    })
}

/// An entry of the section table (C.2).
struct Entry {
    id: u32,
    offset: usize,
    length: usize,
    crc: u32,
}

impl Entry {
    fn range(&self) -> core::ops::Range<usize> {
        self.offset..self.offset + self.length
    }
}

/// The header's checks and the section table (C.1, C.2).
fn section_table(bytes: &[u8]) -> Result<Vec<Entry>, TraceError> {
    if bytes.len() < HEADER_BYTES {
        return Err(TraceError::Truncated("the header"));
    }
    if bytes[..8] != MAGIC {
        return Err(TraceError::NotATrace);
    }
    let mut header = Cursor::new(&bytes[..HEADER_BYTES], "header");
    header.skip(8)?;
    let header_crc = header.u32()?;
    let version = header.u16()?;
    if version != FORMAT_VERSION {
        return Err(TraceError::UnsupportedVersion(version));
    }
    header.skip(2)?;
    let total = header.u64()?;
    let count = header.u32()? as usize;
    header.skip(4)?;
    let table = header.u64()?;
    if total != bytes.len() as u64 {
        return Err(if total > bytes.len() as u64 {
            TraceError::Truncated("the file")
        } else {
            malformed_header("its total length is not the file's")
        });
    }
    let table = usize::try_from(table).map_err(|_| TraceError::Truncated("the section table"))?;
    let table_end = count
        .checked_mul(SECTION_ENTRY_BYTES)
        .and_then(|bytes| bytes.checked_add(table))
        .filter(|end| *end <= bytes.len() && table >= HEADER_BYTES)
        .ok_or(TraceError::Truncated("the section table"))?;
    if crc32::checksum(&bytes[12..table_end]) != header_crc {
        return Err(TraceError::HeaderDamaged);
    }
    let mut cursor = Cursor::new(&bytes[table..table_end], "section table");
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let id = cursor.u32()?;
        cursor.skip(4)?;
        let offset = cursor.u64()?;
        let length = cursor.u64()?;
        let crc = cursor.u32()?;
        cursor.skip(4)?;
        let (Ok(offset), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
            return Err(malformed_header("a section lies outside the file"));
        };
        if offset % 8 != 0
            || offset
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
        {
            return Err(malformed_header("a section lies outside the file"));
        }
        entries.push(Entry {
            id,
            offset,
            length,
            crc,
        });
    }
    Ok(entries)
}

/// `META` (C.3).
fn read_meta(bytes: &[u8], strings: &[u8]) -> Result<Meta, TraceError> {
    let mut at = Cursor::new(bytes, "META");
    let image_sha256 = at.array()?;
    let sources_hash = at.array()?;
    let target = at.u32()?;
    let end = TraceEnd::from_code(at.u32()?).ok_or(malformed(section::META, "unknown end"))?;
    let cycle_interval_ns = at.u64()?;
    let first_cycle = at.u64()?;
    let cycle_count = at.u64()?;
    let started_utc_ms = at.i64()?;
    let mut text = || -> Result<String, TraceError> { pooled(strings, at.u32()?, section::META) };
    let runtime_version = text()?;
    let runtime_id = text()?;
    let platform = text()?;
    let resource = text()?;
    let task = text()?;
    let origin =
        TraceOrigin::from_code(at.u32()?).ok_or(malformed(section::META, "unknown origin"))?;
    Ok(Meta {
        image_sha256,
        sources_hash,
        target,
        end,
        cycle_interval_ns,
        first_cycle,
        cycle_count,
        started_utc_ms,
        runtime_version,
        runtime_id,
        platform,
        resource,
        task: (!task.is_empty()).then_some(task),
        origin,
    })
}

/// `SYMS` (C.4).
fn read_syms(bytes: &[u8], strings: &[u8]) -> Result<Vec<Located>, TraceError> {
    let mut at = Cursor::new(bytes, "SYMS");
    let count = at.u32()? as usize;
    at.skip(4)?;
    let mut located = Vec::with_capacity(count.min(bytes.len() / 24));
    for _ in 0..count {
        let path = pooled(strings, at.u32()?, section::SYMS)?;
        let type_name = pooled(strings, at.u32()?, section::SYMS)?;
        let area = IoArea::from_code(at.u8()?).ok_or(malformed(section::SYMS, "unknown area"))?;
        let bit = bit(at.u8()?, "SYMS")?;
        at.skip(2)?;
        let offset = at.u32()?;
        let size = at.u32()?;
        at.skip(4)?;
        located.push(Located {
            path,
            type_name,
            area,
            bit,
            offset,
            size,
        });
    }
    Ok(located)
}

/// `STAT` (C.5).
fn read_stat(bytes: &[u8]) -> Result<StartState, TraceError> {
    let mut at = Cursor::new(bytes, "STAT");
    let count = at.u32()? as usize;
    at.skip(4)?;
    let mut areas = Vec::with_capacity(count.min(16));
    for _ in 0..count {
        let id = at.u32()?;
        let length = at.u32()? as usize;
        areas.push((id, at.take(length)?.to_vec()));
        at.align()?;
    }
    let forces = read_changes(&mut at)?;
    Ok(StartState { areas, forces })
}

/// `CYCL` (C.6).
fn read_cycle(bytes: &[u8]) -> Result<Cycle, TraceError> {
    let mut at = Cursor::new(bytes, "CYCL");
    let time_ns = at.i64()?;
    let activations = at.u32()? as usize;
    let changes = at.u32()? as usize;
    let readings = at.u32()? as usize;
    let flags = at.u32()?;
    let mut cycle = Cycle {
        time_ns,
        ..Cycle::default()
    };
    for _ in 0..activations {
        let task = at.u32()?;
        at.skip(4)?;
        cycle.activations.push(Activation {
            task,
            now_ns: at.i64()?,
        });
    }
    for _ in 0..readings {
        cycle.wall_clock_ms.push(at.i64()?);
    }
    for _ in 0..changes {
        cycle.changes.push(read_change(&mut at)?);
    }
    cycle.inputs = at.block()?;
    cycle.outputs = at.block()?;
    if flags & 1 != 0 {
        cycle.fault = Some(CycleFault {
            code: at.u32()?,
            pou: at.u32()?,
            statement: at.u32()?,
        });
        at.skip(4)?;
    }
    if !at.at_end() {
        return Err(malformed(section::CYCL, "bytes follow the cycle's record"));
    }
    Ok(cycle)
}

/// A count of changes and the changes, as `STAT` holds its forces.
fn read_changes(at: &mut Cursor<'_>) -> Result<Vec<Change>, TraceError> {
    let count = at.u32()? as usize;
    at.skip(4)?;
    (0..count).map(|_| read_change(at)).collect()
}

/// A change (C.6).
fn read_change(at: &mut Cursor<'_>) -> Result<Change, TraceError> {
    let section = at.section;
    let kind = at.u8()?;
    let source = ChangeSource::from_code(at.u8()?).ok_or(bad(section, "unknown source"))?;
    at.skip(2)?;
    let length = at.u32()? as usize;
    let mut data = Cursor::new(at.take(length)?, section);
    at.align()?;
    let kind = match kind {
        1 | 2 => {
            let path = data.text()?;
            let type_name = data.text()?;
            let value = data.bytes32()?;
            if kind == 1 {
                ChangeKind::VariableWrite {
                    path,
                    type_name,
                    value,
                }
            } else {
                ChangeKind::VariableForce {
                    path,
                    type_name,
                    value,
                }
            }
        }
        3 => ChangeKind::VariableRelease { path: data.text()? },
        4..=6 => {
            let area = IoArea::from_code(data.u8()?).ok_or(bad(section, "unknown area"))?;
            let bit = bit(data.u8()?, section)?;
            data.skip(2)?;
            let offset = data.u32()?;
            match kind {
                4 => ChangeKind::AddressWrite {
                    area,
                    bit,
                    offset,
                    bytes: data.bytes32()?,
                },
                5 => ChangeKind::AddressForce {
                    area,
                    bit,
                    offset,
                    bytes: data.bytes32()?,
                },
                _ => ChangeKind::AddressRelease {
                    area,
                    bit,
                    offset,
                    size: data.u32()?,
                },
            }
        }
        7 => {
            let instance = data.text()?;
            let interface = data.text()?;
            let method = data.text()?;
            let count = data.u16()?;
            let mut inputs = Vec::with_capacity(usize::from(count));
            for _ in 0..count {
                inputs.push((data.text()?, data.text()?));
            }
            ChangeKind::MethodCall {
                instance,
                interface,
                method,
                inputs,
            }
        }
        _ => return Err(bad(section, "unknown change kind")),
    };
    if !data.at_end() {
        return Err(bad(
            section,
            "a change's data is longer than its kind holds",
        ));
    }
    Ok(Change { source, kind })
}

/// The bit of a located variable or an address: 0 to 7, or 255 for whole bytes.
fn bit(code: u8, section: &'static str) -> Result<Option<u8>, TraceError> {
    match code {
        0..=7 => Ok(Some(code)),
        u8::MAX => Ok(None),
        _ => Err(bad(section, "a bit is not 0 to 7 or 255")),
    }
}

/// The NUL-terminated string of `STRS` at `offset`.
fn pooled(strings: &[u8], offset: u32, section: u32) -> Result<String, TraceError> {
    let start = offset as usize;
    let text = strings
        .get(start..)
        .and_then(|rest| {
            rest.iter()
                .position(|byte| *byte == 0)
                .map(|end| &rest[..end])
        })
        .ok_or(malformed(section, "a string lies outside the string pool"))?;
    core::str::from_utf8(text)
        .map(String::from)
        .map_err(|_| malformed(section, "a string is not UTF-8"))
}

/// A malformed section, by its name.
const fn bad(section: &'static str, reason: &'static str) -> TraceError {
    TraceError::Malformed { section, reason }
}

/// A malformed section, by its id.
pub(super) const fn malformed(section: u32, reason: &'static str) -> TraceError {
    bad(name(section), reason)
}

const fn malformed_header(reason: &'static str) -> TraceError {
    bad("header", reason)
}

/// A section's name, as Appendix C, C.2 gives it.
const fn name(id: u32) -> &'static str {
    match id {
        section::META => "META",
        section::STRS => "STRS",
        section::SYMS => "SYMS",
        section::STAT => "STAT",
        section::CYCL => "CYCL",
        section::IMAGE_AREAS => "IAREAS",
        section::IMAGE_TASKS => "ITASKS",
        section::IMAGE_IOBIND => "IIOBIND",
        section::IMAGE_SYMBOLS => "ISYMBOLS",
        section::IMAGE_TYPES => "ITYPES",
        section::IMAGE_STRINGS => "ISTRINGS",
        _ => "unknown",
    }
}

/// Reads a section's little-endian values in order; running past its end is a malformed section.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    section: &'static str,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8], section: &'static str) -> Self {
        Self {
            bytes,
            at: 0,
            section,
        }
    }

    const fn at_end(&self) -> bool {
        self.at == self.bytes.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], TraceError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(bad(self.section, "it ends inside a value"))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn skip(&mut self, n: usize) -> Result<(), TraceError> {
        self.take(n).map(|_| ())
    }

    /// Skips the padding to the next multiple of 8 from the section's start, which the writer
    /// always writes.
    fn align(&mut self) -> Result<(), TraceError> {
        self.skip((8 - self.at % 8) % 8)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], TraceError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    fn u8(&mut self) -> Result<u8, TraceError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, TraceError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, TraceError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, TraceError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64, TraceError> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    /// A text inside a cycle: its length in 2 bytes and its UTF-8 bytes.
    fn text(&mut self) -> Result<String, TraceError> {
        let length = usize::from(self.u16()?);
        let bytes = self.take(length)?;
        core::str::from_utf8(bytes)
            .map(String::from)
            .map_err(|_| bad(self.section, "a text is not UTF-8"))
    }

    /// Bytes after their length in 4 bytes.
    fn bytes32(&mut self) -> Result<Vec<u8>, TraceError> {
        let length = self.u32()? as usize;
        Ok(self.take(length)?.to_vec())
    }

    /// A block of a cycle: its length (4), reserved (4), its bytes, padded to 8.
    fn block(&mut self) -> Result<Vec<u8>, TraceError> {
        let length = self.u32()? as usize;
        self.skip(4)?;
        let bytes = self.take(length)?.to_vec();
        self.align()?;
        Ok(bytes)
    }
}
