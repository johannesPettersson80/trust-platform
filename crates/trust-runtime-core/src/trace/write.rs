//! Writes a trace as the `.trusttrace` file (specification 48 Appendix C).

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use super::{
    section, Change, ChangeKind, Cycle, Located, Meta, StartState, Trace, FORMAT_VERSION,
    HEADER_BYTES, MAGIC, SECTION_ENTRY_BYTES,
};
use crate::crc32;

/// The `.trusttrace` file of `trace` (Appendix C).
#[must_use]
pub fn encode(trace: &Trace) -> Vec<u8> {
    let mut strings = StringPool::default();
    let meta = meta_bytes(&trace.meta, &mut strings);
    let syms = syms_bytes(&trace.located, &mut strings);
    let stat = stat_bytes(&trace.start);
    let strs = strings.bytes;
    let cycles: Vec<Vec<u8>> = trace
        .cycles
        .iter()
        .map(|cycle| {
            let mut bytes = Vec::new();
            encode_cycle(cycle, &mut bytes);
            bytes
        })
        .collect();
    let image = &trace.image;
    let mut sections: Vec<(u32, &[u8])> = vec![
        (section::META, &meta),
        (section::STRS, &strs),
        (section::SYMS, &syms),
        (section::STAT, &stat),
        (section::IMAGE_AREAS, &image.areas),
        (section::IMAGE_TASKS, &image.tasks),
        (section::IMAGE_IOBIND, &image.iobind),
        (section::IMAGE_SYMBOLS, &image.symbols),
        (section::IMAGE_TYPES, &image.types),
        (section::IMAGE_STRINGS, &image.strings),
    ];
    sections.extend(cycles.iter().map(|bytes| (section::CYCL, bytes.as_slice())));
    assemble(&sections)
}

/// Appends one cycle's record to `out`, in the layout of a `CYCL` section (Appendix C, C.6).
///
/// Its blocks are padded to 8 bytes from the start of `out`, so `out` holds whole records: its
/// length is a multiple of 8, as every record this module writes is.
pub fn encode_cycle(cycle: &Cycle, out: &mut Vec<u8>) {
    put_i64(out, cycle.time_ns);
    put_u32(out, count(cycle.activations.len()));
    put_u32(out, count(cycle.changes.len()));
    put_u32(out, count(cycle.wall_clock_ms.len()));
    put_u32(out, u32::from(cycle.fault.is_some()));
    for activation in &cycle.activations {
        put_u32(out, activation.task);
        put_u32(out, 0);
        put_i64(out, activation.now_ns);
    }
    for reading in &cycle.wall_clock_ms {
        put_i64(out, *reading);
    }
    for change in &cycle.changes {
        encode_change(change, out);
    }
    put_block(out, &cycle.inputs);
    put_block(out, &cycle.outputs);
    if let Some(fault) = cycle.fault {
        put_u32(out, fault.code);
        put_u32(out, fault.pou);
        put_u32(out, fault.statement);
        put_u32(out, 0);
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
    out.extend_from_slice(&MAGIC);
    put_u32(&mut out, 0); // the header's CRC-32, filled in below
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&[0; 2]);
    put_u64(&mut out, end as u64);
    put_u32(&mut out, count(sections.len()));
    put_u32(&mut out, 0);
    put_u64(&mut out, HEADER_BYTES as u64);
    out.extend_from_slice(&[0; 24]);
    for ((id, bytes), offset) in sections.iter().zip(&offsets) {
        put_u32(&mut out, *id);
        put_u32(&mut out, 0);
        put_u64(&mut out, *offset as u64);
        put_u64(&mut out, bytes.len() as u64);
        put_u32(&mut out, crc32::checksum(bytes));
        put_u32(&mut out, 0);
    }
    for (_, bytes) in sections {
        out.extend_from_slice(bytes);
        pad(&mut out);
    }
    let header_crc = crc32::checksum(&out[12..table_end]);
    out[8..12].copy_from_slice(&header_crc.to_le_bytes());
    out
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
fn stat_bytes(start: &StartState) -> Vec<u8> {
    let mut out = Vec::new();
    put_u32(&mut out, count(start.areas.len()));
    put_u32(&mut out, 0);
    for (id, bytes) in &start.areas {
        put_u32(&mut out, *id);
        put_u32(&mut out, count(bytes.len()));
        out.extend_from_slice(bytes);
        pad(&mut out);
    }
    put_u32(&mut out, count(start.forces.len()));
    put_u32(&mut out, 0);
    for force in &start.forces {
        encode_change(force, &mut out);
    }
    out
}

/// A change (C.6): its kind, its source, the length of its data, and its data, padded.
fn encode_change(change: &Change, out: &mut Vec<u8>) {
    let mut data = Vec::new();
    match &change.kind {
        ChangeKind::VariableWrite {
            path,
            type_name,
            value,
        }
        | ChangeKind::VariableForce {
            path,
            type_name,
            value,
        } => {
            put_text(&mut data, path);
            put_text(&mut data, type_name);
            put_u32(&mut data, count(value.len()));
            data.extend_from_slice(value);
        }
        ChangeKind::VariableRelease { path } => put_text(&mut data, path),
        ChangeKind::AddressWrite {
            area,
            bit,
            offset,
            bytes,
        }
        | ChangeKind::AddressForce {
            area,
            bit,
            offset,
            bytes,
        } => {
            data.push(*area as u8);
            data.push(bit.unwrap_or(u8::MAX));
            data.extend_from_slice(&[0; 2]);
            put_u32(&mut data, *offset);
            put_u32(&mut data, count(bytes.len()));
            data.extend_from_slice(bytes);
        }
        ChangeKind::AddressRelease {
            area,
            bit,
            offset,
            size,
        } => {
            data.push(*area as u8);
            data.push(bit.unwrap_or(u8::MAX));
            data.extend_from_slice(&[0; 2]);
            put_u32(&mut data, *offset);
            put_u32(&mut data, *size);
        }
        ChangeKind::MethodCall {
            instance,
            interface,
            method,
            inputs,
        } => {
            put_text(&mut data, instance);
            put_text(&mut data, interface);
            put_text(&mut data, method);
            let inputs = &inputs[..inputs.len().min(usize::from(u16::MAX))];
            data.extend_from_slice(
                &u16::try_from(inputs.len())
                    .unwrap_or(u16::MAX)
                    .to_le_bytes(),
            );
            for (name, literal) in inputs {
                put_text(&mut data, name);
                put_text(&mut data, literal);
            }
        }
    }
    out.push(change.kind.code());
    out.push(change.source as u8);
    out.extend_from_slice(&[0; 2]);
    put_u32(out, count(data.len()));
    out.extend_from_slice(&data);
    pad(out);
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
    let mut end = text.len().min(usize::from(u16::MAX));
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    out.extend_from_slice(&u16::try_from(end).unwrap_or(u16::MAX).to_le_bytes());
    out.extend_from_slice(&text.as_bytes()[..end]);
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

const fn padded(n: usize) -> usize {
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
