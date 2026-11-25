//! A trace served part by part (specification 48 RU-59, "Memory"; specification 11 §6.8.1,
//! "Cycle traces"). A chip's heap has no room for the whole `.trusttrace` file beside the records
//! it is made of, so a chip keeps an ended recording's records and its copy of the recorded image's
//! tables, and encodes each part that `trace.read` asks for from them. The file it serves is, byte
//! for byte, the one [`Recorder::file_named`] writes.
//!
//! When the recording ends, the trace takes, once: the debugger's name for each path the records
//! hold (a table of the paths alone, so that a part read later names them alike whatever program
//! then runs), each section's length and CRC-32, the header's CRC-32 and the file's SHA-256. A part
//! then encodes the header, the section table's entries and the sections that it overlaps.
//!
//! A fault's trace is served alike from the flight recorder's two recordings that it joins
//! ([`FlightRecorder::fault_served`](super::FlightRecorder::fault_served)): the start state of the
//! first, then the cycles of both, the records of each taken as they are.

use alloc::string::String;
use alloc::vec::Vec;

use trust_runtime_core::crc32::{self, Crc32Stream};
use trust_runtime_core::deployment::Sha256Stream;
use trust_runtime_core::trace::{
    front_sections, header, named_changes, named_cycle, padded, section, section_entry,
    FrontSections, ImageTables, Located, StartParts, TraceEnd, TraceOrigin, HEADER_BYTES,
    SECTION_ENTRY_BYTES,
};

use super::requests::TraceState;
use super::{Identity, Recorder, Tally};

/// The sections ahead of the cycles: `META`, `STRS`, `SYMS`, `STAT` and the image's six tables.
const FIXED_SECTIONS: usize = 10;

/// An ended recording's trace, served part by part (module docs).
#[derive(Debug)]
pub struct ServedTrace {
    front: FrontSections,
    tables: ImageTables,
    /// The records of the recordings the trace joins, in order: a recording's on demand, or the
    /// flight recorder's full recording and then the faulted one, or the faulted one alone.
    records: [Records; 2],
    /// Each path the records hold, and the debugger's name for it, if it gives one.
    renames: Vec<(String, Option<String>)>,
    /// Each cycle's record, renamed: its length and its CRC-32.
    cycles: Vec<(usize, u32)>,
    /// The CRC-32 of each section ahead of the cycles.
    fixed_crcs: [u32; FIXED_SECTIONS],
    len: usize,
    header_crc: u32,
    sha256: [u8; 32],
    end: TraceEnd,
    cycle_count: u64,
}

/// The records a served trace takes from one recording: the cycles' records and where each ends.
#[derive(Debug, Default)]
struct Records {
    bytes: Vec<u8>,
    ends: Vec<usize>,
}

impl Records {
    /// The records of `recorder`, which keeps none after it.
    fn taken(recorder: &mut Recorder) -> Self {
        Self {
            bytes: core::mem::take(&mut recorder.records),
            ends: core::mem::take(&mut recorder.record_ends),
        }
    }

    /// The record of the cycle `cycle`, counted from these records' first.
    fn record(&self, cycle: usize) -> &[u8] {
        let from = cycle.checked_sub(1).map_or(0, |before| self.ends[before]);
        &self.bytes[from..self.ends[cycle]]
    }
}

impl Recorder {
    /// The ended recording as a trace served part by part, each variable's path and each method
    /// call's instance named by `name` as [`Self::file_named`] names them, its image's tables
    /// `tables`. The trace takes the recorder's records without copying them, so the recorder
    /// keeps none and is done with; `None`, and the recorder as it was, while its recording has
    /// not ended.
    pub fn served(
        &mut self,
        identity: &Identity<'_>,
        located: &[Located],
        tables: ImageTables,
        name: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Option<ServedTrace> {
        let tally = Tally {
            cycles: self.cycles(),
            end: self.end?,
            origin: TraceOrigin::OnDemand,
        };
        Some(ServedTrace::joined(
            self, None, identity, located, tables, tally, name,
        ))
    }
}

impl ServedTrace {
    /// The trace that starts with the start state of `start` and holds its cycles and then those
    /// of `later`, which a fault's trace joins to it, with `identity` and the count, end and
    /// origin of `tally`, each variable's path and each method call's instance named by `name`, its
    /// image's tables `tables`. It takes both recordings' records without copying them.
    pub(super) fn joined(
        start: &mut Recorder,
        later: Option<&mut Recorder>,
        identity: &Identity<'_>,
        located: &[Located],
        tables: ImageTables,
        tally: Tally,
        name: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Self {
        let mut renames: Vec<(String, Option<String>)> = Vec::new();
        let mut lookup = |path: &str| -> Option<String> {
            if let Some((_, named)) = renames.iter().find(|(known, _)| known == path) {
                return named.clone();
            }
            let named = name(path);
            renames.push((String::from(path), named.clone()));
            named
        };
        let cycles: Vec<(usize, u32)> = start
            .cycle_records()
            .chain(
                later
                    .as_deref()
                    .into_iter()
                    .flat_map(Recorder::cycle_records),
            )
            .map(|record| {
                let named = named_cycle(record, &mut lookup);
                let bytes = named.as_deref().unwrap_or(record);
                (bytes.len(), crc32::checksum(bytes))
            })
            .collect();
        let forces = named_changes(&start.start_forces, start.start_force_count, &mut lookup)
            .unwrap_or_else(|_| start.start_forces.clone());
        let begin = start.start_with(&forces);
        let front = front_sections(
            &start.meta(identity, tally),
            located,
            &StartParts {
                areas: &begin.areas,
                force_count: begin.force_count,
                forces: begin.forces,
            },
        );
        drop(begin);
        let records = [
            Records::taken(start),
            later.map(Records::taken).unwrap_or_default(),
        ];
        let mut served = Self {
            front,
            tables,
            records,
            renames,
            cycles,
            fixed_crcs: [0; FIXED_SECTIONS],
            len: 0,
            header_crc: 0,
            sha256: [0; 32],
            end: tally.end,
            cycle_count: tally.cycles,
        };
        served.close();
        served
    }

    /// The trace's state, as `trace.read` answers it.
    #[must_use]
    pub fn state(&self) -> TraceState {
        TraceState::Ended {
            reason: self.end,
            cycles: self.cycle_count,
            bytes: self.len,
            sha256: self.sha256,
        }
    }

    /// The bytes of the file.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the file has no bytes, which a trace's file never has.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Copies the file's bytes from `offset` into `out`, as many as `out` holds and the file has,
    /// and gives how many: none from an offset at or past the file's end.
    pub fn part(&self, offset: usize, out: &mut [u8]) -> usize {
        let wanted = out.len().min(self.len.saturating_sub(offset));
        let mut window = Window {
            from: offset,
            out: &mut out[..wanted],
        };
        if wanted > 0 {
            self.encode(&mut window);
        }
        wanted
    }

    /// The length, CRC-32, header CRC and SHA-256 of the file, once its sections are known.
    fn close(&mut self) {
        self.fixed_crcs = core::array::from_fn(|index| crc32::checksum(self.fixed(index)));
        let count = FIXED_SECTIONS + self.cycles.len();
        let mut end = HEADER_BYTES + SECTION_ENTRY_BYTES * count;
        for index in 0..count {
            end = padded(end + self.section_len(index));
        }
        self.len = end;
        let mut crc = Crc32Stream::new();
        crc.update(&header(self.len, count, 0)[12..]);
        self.each_entry(&mut |entry| crc.update(entry));
        self.header_crc = crc.finish();
        let mut sha256 = Sha256Stream::new();
        let mut all = Hashed(&mut sha256);
        self.encode(&mut all);
        self.sha256 = sha256.finish();
    }

    /// Hands the file's bytes, in order, to `sink`, which takes the ones it wants.
    fn encode(&self, sink: &mut dyn Sink) {
        let count = FIXED_SECTIONS + self.cycles.len();
        sink.put(0, &header(self.len, count, self.header_crc));
        let mut at = HEADER_BYTES;
        if sink.wants(at, SECTION_ENTRY_BYTES * count) {
            self.each_entry(&mut |entry| {
                sink.put(at, entry);
                at += SECTION_ENTRY_BYTES;
            });
        } else {
            at += SECTION_ENTRY_BYTES * count;
        }
        for index in 0..count {
            let len = self.section_len(index);
            if sink.wants(at, padded(len)) {
                self.with_section(index, &mut |bytes| sink.put(at, bytes));
                sink.zeros(at + len, padded(len) - len);
            }
            at += padded(len);
        }
    }

    /// Each section's entry of the section table, in order.
    fn each_entry(&self, each: &mut dyn FnMut(&[u8])) {
        let count = FIXED_SECTIONS + self.cycles.len();
        let mut offset = HEADER_BYTES + SECTION_ENTRY_BYTES * count;
        for index in 0..count {
            let len = self.section_len(index);
            each(&section_entry(
                Self::section_id(index),
                offset,
                len,
                self.section_crc(index),
            ));
            offset = padded(offset + len);
        }
    }

    const fn section_id(index: usize) -> u32 {
        match index {
            0 => section::META,
            1 => section::STRS,
            2 => section::SYMS,
            3 => section::STAT,
            4 => section::IMAGE_AREAS,
            5 => section::IMAGE_TASKS,
            6 => section::IMAGE_IOBIND,
            7 => section::IMAGE_SYMBOLS,
            8 => section::IMAGE_TYPES,
            9 => section::IMAGE_STRINGS,
            _ => section::CYCL,
        }
    }

    /// The bytes of a section ahead of the cycles.
    fn fixed(&self, index: usize) -> &[u8] {
        match index {
            0 => &self.front.meta,
            1 => &self.front.strs,
            2 => &self.front.syms,
            3 => &self.front.stat,
            4 => &self.tables.areas,
            5 => &self.tables.tasks,
            6 => &self.tables.iobind,
            7 => &self.tables.symbols,
            8 => &self.tables.types,
            _ => &self.tables.strings,
        }
    }

    fn section_len(&self, index: usize) -> usize {
        match index.checked_sub(FIXED_SECTIONS) {
            None => self.fixed(index).len(),
            Some(cycle) => self.cycles[cycle].0,
        }
    }

    fn section_crc(&self, index: usize) -> u32 {
        match index.checked_sub(FIXED_SECTIONS) {
            None => self.fixed_crcs[index],
            Some(cycle) => self.cycles[cycle].1,
        }
    }

    /// Hands the bytes of the section `index` to `with`: a cycle's record renamed through the
    /// trace's table of names.
    fn with_section(&self, index: usize, with: &mut dyn FnMut(&[u8])) {
        let Some(cycle) = index.checked_sub(FIXED_SECTIONS) else {
            with(self.fixed(index));
            return;
        };
        let [first, second] = &self.records;
        let record = match cycle.checked_sub(first.ends.len()) {
            None => first.record(cycle),
            Some(later) => second.record(later),
        };
        let named = named_cycle(record, &mut |path| {
            self.renames
                .iter()
                .find(|(known, _)| known == path)
                .and_then(|(_, named)| named.clone())
        });
        with(named.as_deref().unwrap_or(record));
    }
}

/// Where the file's bytes go as the trace encodes them.
trait Sink {
    /// Whether any of the `len` bytes from `at` are wanted.
    fn wants(&self, at: usize, len: usize) -> bool;
    /// The bytes `bytes` at the file's offset `at`.
    fn put(&mut self, at: usize, bytes: &[u8]);
    /// `len` zero bytes at the file's offset `at`.
    fn zeros(&mut self, at: usize, len: usize);
}

/// The part of the file from `from` that fills `out`.
struct Window<'o> {
    from: usize,
    out: &'o mut [u8],
}

impl Sink for Window<'_> {
    fn wants(&self, at: usize, len: usize) -> bool {
        at < self.from + self.out.len() && self.from < at + len
    }

    fn put(&mut self, at: usize, bytes: &[u8]) {
        let start = self.from.max(at);
        let end = (self.from + self.out.len()).min(at + bytes.len());
        if start < end {
            self.out[start - self.from..end - self.from]
                .copy_from_slice(&bytes[start - at..end - at]);
        }
    }

    fn zeros(&mut self, at: usize, len: usize) {
        let start = self.from.max(at);
        let end = (self.from + self.out.len()).min(at + len);
        if start < end {
            self.out[start - self.from..end - self.from].fill(0);
        }
    }
}

/// The whole file, into the SHA-256 that `trace.read` gives.
struct Hashed<'h>(&'h mut Sha256Stream);

impl Sink for Hashed<'_> {
    fn wants(&self, _at: usize, _len: usize) -> bool {
        true
    }

    fn put(&mut self, _at: usize, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn zeros(&mut self, _at: usize, len: usize) {
        self.0.update(&[0; 8][..len]);
    }
}

#[cfg(test)]
mod tests;
