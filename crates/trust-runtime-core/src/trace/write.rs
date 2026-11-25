//! Writes a trace as the `.trusttrace` file (specification 48 Appendix C).

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use super::{
    section, Activation, ChangeKind, ChangeSource, Cycle, CycleFault, ImageTables, IoArea, Located,
    Meta, Trace, FORMAT_VERSION, HEADER_BYTES, MAGIC, SECTION_ENTRY_BYTES,
};
use crate::crc32;

/// The `.trusttrace` file of `trace` (Appendix C).
#[must_use]
pub fn encode(trace: &Trace) -> Vec<u8> {
    let records: Vec<Vec<u8>> = trace
        .cycles
        .iter()
        .map(|cycle| {
            let mut bytes = Vec::new();
            encode_cycle(cycle, &mut bytes);
            bytes
        })
        .collect();
    let records: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
    let mut forces = Vec::new();
    for force in &trace.start.forces {
        encode_change(
            force.source,
            &force.kind.borrowed(&mut Vec::new()),
            &mut forces,
        );
    }
    let areas: Vec<(u32, &[u8])> = trace
        .start
        .areas
        .iter()
        .map(|(id, bytes)| (*id, bytes.as_slice()))
        .collect();
    let start = StartParts {
        areas: &areas,
        force_count: trace.start.forces.len(),
        forces: &forces,
    };
    encode_records(&trace.meta, &trace.located, &start, &trace.image, &records)
}

/// The `.trusttrace` file of a trace whose cycles are already records (C.6), as a recorder holds
/// them: the identity, the located variables, the start state, the recorded image's tables, and one
/// `CYCL` section for each record.
#[must_use]
pub fn encode_records(
    meta: &Meta,
    located: &[Located],
    start: &StartParts<'_>,
    image: &ImageTables,
    records: &[&[u8]],
) -> Vec<u8> {
    let front = front_sections(meta, located, start);
    let mut sections: Vec<(u32, &[u8])> = vec![
        (section::META, &front.meta),
        (section::STRS, &front.strs),
        (section::SYMS, &front.syms),
        (section::STAT, &front.stat),
        (section::IMAGE_AREAS, &image.areas),
        (section::IMAGE_TASKS, &image.tasks),
        (section::IMAGE_IOBIND, &image.iobind),
        (section::IMAGE_SYMBOLS, &image.symbols),
        (section::IMAGE_TYPES, &image.types),
        (section::IMAGE_STRINGS, &image.strings),
    ];
    sections.extend(records.iter().map(|record| (section::CYCL, *record)));
    assemble(&sections)
}

/// The start state from its parts (C.5): each area's id and bytes, and the forces in effect as
/// `force_count` changes that [`encode_change`] wrote into `forces`.
#[derive(Debug, Clone, Copy)]
pub struct StartParts<'a> {
    /// Each area's id and bytes.
    pub areas: &'a [(u32, &'a [u8])],
    /// How many forces `forces` holds.
    pub force_count: usize,
    /// The forces, each a change [`encode_change`] wrote.
    pub forces: &'a [u8],
}

/// A cycle's record from its parts (C.6), as a recorder gathers them during a scan.
#[derive(Debug, Clone, Copy)]
pub struct CycleParts<'a> {
    /// The cycle's logical time, in nanoseconds.
    pub time_ns: i64,
    /// Each task activation, in order.
    pub activations: &'a [Activation],
    /// Each reading of the wall clock, in order.
    pub wall_clock_ms: &'a [i64],
    /// How many changes `changes` holds.
    pub change_count: usize,
    /// The changes, each one [`encode_change`] wrote.
    pub changes: &'a [u8],
    /// The input image as the scan sampled it.
    pub inputs: &'a [u8],
    /// The output image the cycle wrote.
    pub outputs: &'a [u8],
    /// The fault that ended the cycle.
    pub fault: Option<CycleFault>,
}

/// Appends one cycle's record to `out`, in the layout of a `CYCL` section (Appendix C, C.6).
///
/// Its blocks are padded to 8 bytes from the start of `out`, so `out` holds whole records: its
/// length is a multiple of 8, as every record this module writes is.
pub fn encode_cycle(cycle: &Cycle, out: &mut Vec<u8>) {
    let mut changes = Vec::new();
    for change in &cycle.changes {
        encode_change(
            change.source,
            &change.kind.borrowed(&mut Vec::new()),
            &mut changes,
        );
    }
    encode_cycle_parts(
        &CycleParts {
            time_ns: cycle.time_ns,
            activations: &cycle.activations,
            wall_clock_ms: &cycle.wall_clock_ms,
            change_count: cycle.changes.len(),
            changes: &changes,
            inputs: &cycle.inputs,
            outputs: &cycle.outputs,
            fault: cycle.fault,
        },
        out,
    );
}

/// Appends the record of `parts` to `out`, which holds whole records (see [`encode_cycle`]). It
/// grows `out` by [`cycle_record_len`] bytes, so that it allocates nothing when `out` has room.
pub fn encode_cycle_parts(parts: &CycleParts<'_>, out: &mut Vec<u8>) {
    put_i64(out, parts.time_ns);
    put_u32(out, count(parts.activations.len()));
    put_u32(out, count(parts.change_count));
    put_u32(out, count(parts.wall_clock_ms.len()));
    put_u32(out, u32::from(parts.fault.is_some()));
    for activation in parts.activations {
        put_u32(out, activation.task);
        put_u32(out, 0);
        put_i64(out, activation.now_ns);
    }
    for reading in parts.wall_clock_ms {
        put_i64(out, *reading);
    }
    out.extend_from_slice(parts.changes);
    put_block(out, parts.inputs);
    put_block(out, parts.outputs);
    if let Some(fault) = parts.fault {
        put_u32(out, fault.code);
        put_u32(out, fault.pou);
        put_u32(out, fault.statement);
        put_u32(out, 0);
    }
}

/// The bytes [`encode_cycle_parts`] appends for `parts`.
#[must_use]
pub fn cycle_record_len(parts: &CycleParts<'_>) -> usize {
    24 + 16 * parts.activations.len()
        + 8 * parts.wall_clock_ms.len()
        + parts.changes.len()
        + 8
        + padded(parts.inputs.len())
        + 8
        + padded(parts.outputs.len())
        + if parts.fault.is_some() { 16 } else { 0 }
}

/// A change to record, borrowed (C.6): what a [`ChangeKind`] holds, without owning it, so that a
/// recorder writes a change during a scan without allocating.
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum ChangeRef<'a> {
    VariableWrite {
        path: &'a str,
        type_name: &'a str,
        value: &'a [u8],
    },
    VariableForce {
        path: &'a str,
        type_name: &'a str,
        value: &'a [u8],
    },
    VariableRelease {
        path: &'a str,
    },
    AddressWrite {
        area: IoArea,
        bit: Option<u8>,
        offset: u32,
        bytes: &'a [u8],
    },
    AddressForce {
        area: IoArea,
        bit: Option<u8>,
        offset: u32,
        bytes: &'a [u8],
    },
    AddressRelease {
        area: IoArea,
        bit: Option<u8>,
        offset: u32,
        size: u32,
    },
    MethodCall {
        instance: &'a str,
        interface: &'a str,
        method: &'a str,
        /// Each input's name and Structured Text literal.
        inputs: &'a [(&'a str, &'a str)],
        /// Each reading of the wall clock the call made, in order.
        readings: &'a [i64],
    },
}

impl ChangeKind {
    /// The change borrowed, as [`encode_change`] takes it, its method call's inputs gathered into
    /// `pairs`.
    pub fn borrowed<'a>(&'a self, pairs: &'a mut Vec<(&'a str, &'a str)>) -> ChangeRef<'a> {
        match self {
            Self::VariableWrite {
                path,
                type_name,
                value,
            } => ChangeRef::VariableWrite {
                path,
                type_name,
                value,
            },
            Self::VariableForce {
                path,
                type_name,
                value,
            } => ChangeRef::VariableForce {
                path,
                type_name,
                value,
            },
            Self::VariableRelease { path } => ChangeRef::VariableRelease { path },
            Self::AddressWrite {
                area,
                bit,
                offset,
                bytes,
            } => ChangeRef::AddressWrite {
                area: *area,
                bit: *bit,
                offset: *offset,
                bytes,
            },
            Self::AddressForce {
                area,
                bit,
                offset,
                bytes,
            } => ChangeRef::AddressForce {
                area: *area,
                bit: *bit,
                offset: *offset,
                bytes,
            },
            Self::AddressRelease {
                area,
                bit,
                offset,
                size,
            } => ChangeRef::AddressRelease {
                area: *area,
                bit: *bit,
                offset: *offset,
                size: *size,
            },
            Self::MethodCall {
                instance,
                interface,
                method,
                inputs,
                readings,
            } => {
                pairs.extend(
                    inputs
                        .iter()
                        .map(|(name, literal)| (name.as_str(), literal.as_str())),
                );
                ChangeRef::MethodCall {
                    instance,
                    interface,
                    method,
                    inputs: pairs,
                    readings,
                }
            }
        }
    }
}

/// Appends a change to `out` (C.6): its kind, its source, the length of its data, and its data,
/// padded to 8 bytes from the start of `out`, which holds whole changes. It grows `out` by
/// [`change_len`] bytes, so that it allocates nothing when `out` has room.
pub fn encode_change(source: ChangeSource, change: &ChangeRef<'_>, out: &mut Vec<u8>) {
    out.push(kind_code(change));
    out.push(source as u8);
    out.extend_from_slice(&[0; 2]);
    put_u32(out, count(data_len(change)));
    match change {
        ChangeRef::VariableWrite {
            path,
            type_name,
            value,
        }
        | ChangeRef::VariableForce {
            path,
            type_name,
            value,
        } => {
            put_text(out, path);
            put_text(out, type_name);
            put_u32(out, count(value.len()));
            out.extend_from_slice(value);
        }
        ChangeRef::VariableRelease { path } => put_text(out, path),
        ChangeRef::AddressWrite {
            area,
            bit,
            offset,
            bytes,
        }
        | ChangeRef::AddressForce {
            area,
            bit,
            offset,
            bytes,
        } => {
            out.push(*area as u8);
            out.push(bit.unwrap_or(u8::MAX));
            out.extend_from_slice(&[0; 2]);
            put_u32(out, *offset);
            put_u32(out, count(bytes.len()));
            out.extend_from_slice(bytes);
        }
        ChangeRef::AddressRelease {
            area,
            bit,
            offset,
            size,
        } => {
            out.push(*area as u8);
            out.push(bit.unwrap_or(u8::MAX));
            out.extend_from_slice(&[0; 2]);
            put_u32(out, *offset);
            put_u32(out, *size);
        }
        ChangeRef::MethodCall {
            instance,
            interface,
            method,
            inputs,
            readings,
        } => {
            put_text(out, instance);
            put_text(out, interface);
            put_text(out, method);
            let inputs = &inputs[..inputs.len().min(usize::from(u16::MAX))];
            out.extend_from_slice(
                &u16::try_from(inputs.len())
                    .unwrap_or(u16::MAX)
                    .to_le_bytes(),
            );
            for (name, literal) in inputs {
                put_text(out, name);
                put_text(out, literal);
            }
            let readings = &readings[..readings.len().min(usize::from(u16::MAX))];
            out.extend_from_slice(
                &u16::try_from(readings.len())
                    .unwrap_or(u16::MAX)
                    .to_le_bytes(),
            );
            for reading in readings {
                out.extend_from_slice(&reading.to_le_bytes());
            }
        }
    }
    pad(out);
}

/// The bytes [`encode_change`] appends for `change`.
#[must_use]
pub fn change_len(change: &ChangeRef<'_>) -> usize {
    padded(8 + data_len(change))
}

/// The length of a change's data (C.6), before its padding.
fn data_len(change: &ChangeRef<'_>) -> usize {
    match change {
        ChangeRef::VariableWrite {
            path,
            type_name,
            value,
        }
        | ChangeRef::VariableForce {
            path,
            type_name,
            value,
        } => text_len(path) + text_len(type_name) + 4 + value.len(),
        ChangeRef::VariableRelease { path } => text_len(path),
        ChangeRef::AddressWrite { bytes, .. } | ChangeRef::AddressForce { bytes, .. } => {
            12 + bytes.len()
        }
        ChangeRef::AddressRelease { .. } => 12,
        ChangeRef::MethodCall {
            instance,
            interface,
            method,
            inputs,
            readings,
        } => {
            let inputs = &inputs[..inputs.len().min(usize::from(u16::MAX))];
            text_len(instance)
                + text_len(interface)
                + text_len(method)
                + 2
                + inputs
                    .iter()
                    .map(|(name, literal)| text_len(name) + text_len(literal))
                    .sum::<usize>()
                + 2
                + 8 * readings.len().min(usize::from(u16::MAX))
        }
    }
}

const fn kind_code(change: &ChangeRef<'_>) -> u8 {
    match change {
        ChangeRef::VariableWrite { .. } => 1,
        ChangeRef::VariableForce { .. } => 2,
        ChangeRef::VariableRelease { .. } => 3,
        ChangeRef::AddressWrite { .. } => 4,
        ChangeRef::AddressForce { .. } => 5,
        ChangeRef::AddressRelease { .. } => 6,
        ChangeRef::MethodCall { .. } => 7,
    }
}

/// The header, the section table and the sections, each at a multiple of 8 bytes (C.1, C.2).
fn assemble(sections: &[(u32, &[u8])]) -> Vec<u8> {
    let table_end = HEADER_BYTES + SECTION_ENTRY_BYTES * sections.len();
    let mut offsets = Vec::with_capacity(sections.len());
    let mut end = table_end;
    for (_, bytes) in sections {
        offsets.push(end);
        end = padded(end + bytes.len());
    }
    let mut out = Vec::with_capacity(end);
    // The header's CRC-32 is filled in below, once the table is written.
    out.extend_from_slice(&header(end, sections.len(), 0));
    for ((id, bytes), offset) in sections.iter().zip(&offsets) {
        out.extend_from_slice(&section_entry(
            *id,
            *offset,
            bytes.len(),
            crc32::checksum(bytes),
        ));
    }
    for (_, bytes) in sections {
        out.extend_from_slice(bytes);
        pad(&mut out);
    }
    let header_crc = crc32::checksum(&out[12..table_end]);
    out[8..12].copy_from_slice(&header_crc.to_le_bytes());
    out
}

/// The file's header (C.1) for a file of `total_len` bytes with `section_count` sections, its
/// section table right after it, and the CRC-32 `header_crc` of bytes 12 to the table's end.
#[must_use]
pub fn header(total_len: usize, section_count: usize, header_crc: u32) -> [u8; HEADER_BYTES] {
    let mut out = [0; HEADER_BYTES];
    out[..8].copy_from_slice(&MAGIC);
    out[8..12].copy_from_slice(&header_crc.to_le_bytes());
    out[12..14].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    out[16..24].copy_from_slice(&(total_len as u64).to_le_bytes());
    out[24..28].copy_from_slice(&count(section_count).to_le_bytes());
    out[32..40].copy_from_slice(&(HEADER_BYTES as u64).to_le_bytes());
    out
}

/// A section's entry in the section table (C.2): its id, its offset in the file, its length and
/// the CRC-32 of its bytes.
#[must_use]
pub fn section_entry(id: u32, offset: usize, len: usize, crc: u32) -> [u8; SECTION_ENTRY_BYTES] {
    let mut out = [0; SECTION_ENTRY_BYTES];
    out[..4].copy_from_slice(&id.to_le_bytes());
    out[8..16].copy_from_slice(&(offset as u64).to_le_bytes());
    out[16..24].copy_from_slice(&(len as u64).to_le_bytes());
    out[24..28].copy_from_slice(&crc.to_le_bytes());
    out
}

/// The sections of a file ahead of the recorded image's tables: `META`, `STRS`, `SYMS` and `STAT`
/// (C.2), which a trace served part by part keeps whole ([`front_sections`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontSections {
    /// `META` (C.3).
    pub meta: Vec<u8>,
    /// `STRS`: the string pool of `META` and `SYMS`.
    pub strs: Vec<u8>,
    /// `SYMS` (C.4).
    pub syms: Vec<u8>,
    /// `STAT` (C.5).
    pub stat: Vec<u8>,
}

/// The front sections of the file of a trace of `meta`, `located` and `start`, as
/// [`encode_records`] writes them.
#[must_use]
pub fn front_sections(meta: &Meta, located: &[Located], start: &StartParts<'_>) -> FrontSections {
    let mut strings = StringPool::default();
    let meta = meta_bytes(meta, &mut strings);
    let syms = syms_bytes(located, &mut strings);
    let stat = stat_bytes(start);
    FrontSections {
        meta,
        strs: strings.bytes,
        syms,
        stat,
    }
}

/// `META` (C.3): 128 bytes, its texts in the string pool.
fn meta_bytes(meta: &Meta, strings: &mut StringPool) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&meta.image_sha256);
    out.extend_from_slice(&meta.sources_hash);
    put_u32(&mut out, meta.target);
    put_u32(&mut out, meta.end as u32);
    put_u64(&mut out, meta.cycle_interval_ns);
    put_u64(&mut out, meta.first_cycle);
    put_u64(&mut out, meta.cycle_count);
    put_i64(&mut out, meta.started_utc_ms);
    put_u32(&mut out, strings.add(&meta.runtime_version));
    put_u32(&mut out, strings.add(&meta.runtime_id));
    put_u32(&mut out, strings.add(&meta.platform));
    put_u32(&mut out, strings.add(&meta.resource));
    put_u32(
        &mut out,
        meta.task.as_deref().map_or(0, |task| strings.add(task)),
    );
    put_u32(&mut out, meta.origin as u32);
    out
}

/// `SYMS` (C.4): a count, then 24 bytes for each located variable.
fn syms_bytes(located: &[Located], strings: &mut StringPool) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 24 * located.len());
    put_u32(&mut out, count(located.len()));
    put_u32(&mut out, 0);
    for variable in located {
        put_u32(&mut out, strings.add(&variable.path));
        put_u32(&mut out, strings.add(&variable.type_name));
        out.push(variable.area as u8);
        out.push(variable.bit.unwrap_or(u8::MAX));
        out.extend_from_slice(&[0; 2]);
        put_u32(&mut out, variable.offset);
        put_u32(&mut out, variable.size);
        put_u32(&mut out, 0);
    }
    out
}

/// `STAT` (C.5): the areas, then the forces in effect.
fn stat_bytes(start: &StartParts<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    put_u32(&mut out, count(start.areas.len()));
    put_u32(&mut out, 0);
    for (id, bytes) in start.areas {
        put_u32(&mut out, *id);
        put_u32(&mut out, count(bytes.len()));
        out.extend_from_slice(bytes);
        pad(&mut out);
    }
    put_u32(&mut out, count(start.force_count));
    put_u32(&mut out, 0);
    out.extend_from_slice(start.forces);
    out
}

/// The texts of `META` and `SYMS`, each once, NUL-terminated, the empty string at offset 0.
#[derive(Default)]
struct StringPool {
    bytes: Vec<u8>,
    offsets: BTreeMap<alloc::string::String, u32>,
}

impl StringPool {
    fn add(&mut self, text: &str) -> u32 {
        if self.bytes.is_empty() {
            self.bytes.push(0);
        }
        if text.is_empty() {
            return 0;
        }
        if let Some(offset) = self.offsets.get(text) {
            return *offset;
        }
        // A name never holds NUL; one that did would end at its first NUL when read.
        let offset = count(self.bytes.len());
        self.bytes.extend_from_slice(text.as_bytes());
        self.bytes.push(0);
        self.offsets.insert(text.into(), offset);
        offset
    }
}

/// A text inside a cycle (C.6): its length in 2 bytes and its UTF-8 bytes. A text longer than
/// 65,535 bytes, which no runtime name is, is cut at the last character that fits.
fn put_text(out: &mut Vec<u8>, text: &str) {
    let end = text_end(text);
    out.extend_from_slice(&u16::try_from(end).unwrap_or(u16::MAX).to_le_bytes());
    out.extend_from_slice(&text.as_bytes()[..end]);
}

/// Where [`put_text`] cuts `text`.
fn text_end(text: &str) -> usize {
    let mut end = text.len().min(usize::from(u16::MAX));
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// The bytes [`put_text`] writes for `text`.
fn text_len(text: &str) -> usize {
    2 + text_end(text)
}

/// A length-prefixed block padded to 8 bytes: an image of a cycle.
fn put_block(out: &mut Vec<u8>, bytes: &[u8]) {
    put_u32(out, count(bytes.len()));
    put_u32(out, 0);
    out.extend_from_slice(bytes);
    pad(out);
}

/// A count or a length as the file's 4 bytes. Nothing a runtime records comes near 4 GiB (a process
/// image is at most 16 MiB); a larger one would read back as malformed, never wrap silently.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `n` rounded up to a multiple of 8, as each section is padded (C.2).
#[must_use]
pub const fn padded(n: usize) -> usize {
    (n + 7) & !7
}

fn pad(out: &mut Vec<u8>) {
    out.resize(padded(out.len()), 0);
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes());
}
