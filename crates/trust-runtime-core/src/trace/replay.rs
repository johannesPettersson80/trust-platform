//! What a replay of a cycle trace needs from the trace and from the build that replays it
//! (specification 48 RU-60): the recorded tasks' names, whether the build lays its memory out as
//! the recorded program did, the recorded program's image and memory that another build carries
//! its values from, and the located outputs whose bytes differ after a cycle.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use smol_str::SmolStr;

use super::{section, ImageTables, IoArea, Located, Trace, TraceError};
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::native::image::{
    EntryRecord, FileRecord, Image, ImageWriter, MemoryFigure, Record, RetainRecord, SectionId,
    StmtRecord, Target, TaskRecord, TABLE_PREFIX_BYTES,
};
use crate::native::memory::ProgramMemory;
use crate::native::reflect::resolve_path;
use crate::native::{read_value, Types};
use crate::value::Value;

/// The names of the recorded image's tasks, in the order of its task table, which a cycle's
/// activations index (Appendix C, C.6): the build's tasks of the same names run them.
///
/// # Errors
///
/// [`TraceError::Malformed`] for a task table its records do not fit, or a task whose name the
/// trace's string pool does not hold.
pub fn task_names(tables: &ImageTables) -> Result<Vec<String>, TraceError> {
    let malformed = |reason| super::read::malformed(section::IMAGE_TASKS, reason);
    let count = tables
        .tasks
        .first_chunk::<4>()
        .map(|count| u32::from_le_bytes(*count))
        .ok_or(malformed("it has no record count"))?;
    let count = usize::try_from(count).map_err(|_| malformed("its record count is too large"))?;
    (0..count)
        .map(|index| {
            let start = index
                .checked_mul(TaskRecord::BYTES)
                .and_then(|offset| offset.checked_add(TABLE_PREFIX_BYTES))
                .ok_or(malformed("its record count is too large"))?;
            let record = tables
                .tasks
                .get(start..start + TaskRecord::BYTES)
                .ok_or(malformed("its records do not fit it"))?;
            let name = TaskRecord::read(record).name;
            pooled(&tables.strings, name).ok_or(malformed("a task's name is not in the pool"))
        })
        .collect()
}

/// Whether `image` lays its memory out as the recorded program did: its area, symbol and type
/// tables equal the trace's, byte for byte, so that it takes the recorded memory as it is (RU-60,
/// "The start").
#[must_use]
pub fn same_layout(tables: &ImageTables, image: &Image<'_>) -> bool {
    tables.areas == image.section_bytes(SectionId::Areas)
        && tables.symbols == image.section_bytes(SectionId::Symbols)
        && tables.types == image.section_bytes(SectionId::Types)
}

/// The recorded program's image, put together from the trace's tables (Appendix C, C.2): its area,
/// task, I/O binding, symbol and type tables and its string pool, with no module, and empty entry,
/// statement, retain and file tables, no starting memory and an empty memory figure, which an image
/// must hold (specification 44 §5.3) and which reading the memory does not use. A replay reads the
/// recorded memory by it, and a build whose memory is laid out otherwise carries its values from
/// it as an online change carries a running program's (RU-60, "The start"; specification 44 §12.2).
///
/// # Errors
///
/// [`TraceError::Malformed`] for a target no build has, and for tables that do not lay out as an
/// image.
pub fn recorded_image(trace: &Trace) -> Result<Vec<u8>, TraceError> {
    let malformed = |reason| super::read::malformed(section::META, reason);
    let target = Target::from_raw(trace.meta.target).ok_or(malformed("it names no target"))?;
    let interface = u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap_or(u16::MAX);
    let mut writer = ImageWriter::new(target, interface);
    let tables = &trace.image;
    for (id, bytes) in [
        (SectionId::Areas, &tables.areas),
        (SectionId::Tasks, &tables.tasks),
        (SectionId::IoBind, &tables.iobind),
        (SectionId::Symbols, &tables.symbols),
        (SectionId::Types, &tables.types),
        (SectionId::Strings, &tables.strings),
    ] {
        writer.section(id, bytes.clone());
    }
    writer.table::<EntryRecord>(&[], &[]);
    writer.section(SectionId::Init, Vec::new());
    writer.table::<StmtRecord>(&[], &[]);
    writer.table::<RetainRecord>(&[], &[]);
    writer.table::<FileRecord>(&[], &[]);
    writer.section(SectionId::MemFig, MemoryFigure::default().bytes().to_vec());
    writer
        .finish()
        .map_err(|_| super::read::malformed(section::IMAGE_AREAS, "its tables do not lay out"))
}

/// The recorded program's memory at the trace's start, read by its image ([`recorded_image`]):
/// what a build whose memory is laid out otherwise carries its values from, as an online change
/// carries a running program's (RU-60, "The start"; specification 44 §12.2). It is only read.
pub struct RecordedMemory<'t> {
    image: Image<'t>,
    areas: &'t [(u32, Vec<u8>)],
}

impl<'t> RecordedMemory<'t> {
    /// The memory of the recorded program of `image`, whose areas the trace's start state holds,
    /// each with its id.
    #[must_use]
    pub fn new(image: Image<'t>, areas: &'t [(u32, Vec<u8>)]) -> Self {
        Self { image, areas }
    }
}

impl ProgramMemory for RecordedMemory<'_> {
    fn image(&self) -> Image<'_> {
        self.image
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        let held = self
            .areas
            .iter()
            .find(|(area, _)| *area == u32::from(id))
            .and_then(|(_, bytes)| {
                offset
                    .checked_add(out.len())
                    .and_then(|end| bytes.get(offset..end))
            });
        match held {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    fn write_area(&mut self, _id: u8, _offset: usize, _data: &[u8]) -> bool {
        false
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::ControlError(SmolStr::new_static(
            "a trace's recorded memory is only read",
        )))
    }
}

/// Whether `image` names the variable `path`, as a debugger's path names it (RU-60, "Each cycle":
/// a change of a variable the build lacks is skipped).
#[must_use]
pub fn names_variable(image: &Image<'_>, path: &str) -> bool {
    resolve_path(image, path).is_ok()
}

/// A recorded value as a request gave it to the runtime: an integer or a bit string as the integer
/// literal that the checked write types for the variable, as `set` types a literal, so that a
/// build which changed the variable's type takes it when the new type holds it (RU-60, "Each
/// cycle": the changes are applied "as the runtime applied them"); every other value as it is,
/// and a number past the literal's range too.
#[must_use]
pub fn as_requested(value: Value) -> Value {
    let literal = match value {
        Value::SInt(number) => Some(i64::from(number)),
        Value::Int(number) => Some(i64::from(number)),
        Value::DInt(number) => Some(i64::from(number)),
        Value::LInt(number) => Some(number),
        Value::USInt(number) | Value::Byte(number) => Some(i64::from(number)),
        Value::UInt(number) | Value::Word(number) => Some(i64::from(number)),
        Value::UDInt(number) | Value::DWord(number) => Some(i64::from(number)),
        Value::ULInt(number) | Value::LWord(number) => i64::try_from(number).ok(),
        _ => None,
    };
    literal.map_or(value, Value::LInt)
}

/// The located output variables whose bytes differ between the trace's output image of a cycle
/// and the build's after the same cycle, in the order of `located`: a variable of one bit by that
/// bit, every other by its bytes (RU-60, "Each cycle"). A variable that one image does not hold
/// differs.
#[must_use]
pub fn differing_outputs<'l>(
    located: &'l [Located],
    trace: &[u8],
    build: &[u8],
) -> Vec<&'l Located> {
    located
        .iter()
        .filter(|variable| variable.area == IoArea::Output)
        .filter(|variable| located_bytes(variable, trace) != located_bytes(variable, build))
        .collect()
}

/// The bytes of `variable` in the process image `image`, its bit alone, 0 or 1, for a variable of
/// one bit; `None` where the image does not hold it.
#[must_use]
pub fn located_bytes(variable: &Located, image: &[u8]) -> Option<Vec<u8>> {
    let start = usize::try_from(variable.offset).ok()?;
    match variable.bit {
        Some(bit) => image
            .get(start)
            .map(|byte| alloc::vec![byte >> (bit & 7) & 1]),
        None => {
            let end = start.checked_add(usize::try_from(variable.size).ok()?)?;
            image.get(start..end).map(<[u8]>::to_vec)
        }
    }
}

/// The string at `offset` of the image's string pool `pool`, without its NUL; `None` past the pool
/// or for one that is not UTF-8.
fn pooled(pool: &[u8], offset: u32) -> Option<String> {
    let rest = pool.get(usize::try_from(offset).ok()?..)?;
    let end = rest.iter().position(|byte| *byte == 0)?;
    core::str::from_utf8(rest.get(..end)?)
        .ok()
        .map(ToString::to_string)
}

/// The value that `bytes`, the bytes of `variable` that [`located_bytes`] gives, hold as the
/// variable of the same path in `image` holds a value, for a replay's report (RU-60, "The
/// result"); `None` for a path the image does not name or bytes that are no value of its type.
#[must_use]
pub fn located_value(image: &Image<'_>, variable: &Located, bytes: &[u8]) -> Option<Value> {
    if variable.bit.is_some() {
        return bytes.first().map(|bit| Value::Bool(*bit == 1));
    }
    path_value(image, &variable.path, bytes)
}

/// The value that `bytes`, a variable's bytes at its declared size as a trace records a write or a
/// force (Appendix C, C.6), hold as the variable `path` of `image` holds a value: what a replay
/// writes into the build (RU-60). `None` for a path the image does not name or bytes that are no
/// value of its type.
#[must_use]
pub fn path_value(image: &Image<'_>, path: &str, bytes: &[u8]) -> Option<Value> {
    let at = resolve_path(image, path).ok()?;
    let types = Types::of(image).ok()?;
    read_value(&types, bytes, 0, at.type_index).ok()
}

/// The address of `variable` as IEC 61131-3 writes one (§6.5.5.2): `%QX0.1`, `%IW4`, `%MB9`.
#[must_use]
pub fn located_address(variable: &Located) -> String {
    let area = match variable.area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    match variable.bit {
        Some(bit) => format!("%{area}X{}.{bit}", variable.offset),
        None => {
            let size = match variable.size {
                2 => 'W',
                4 => 'D',
                8 => 'L',
                _ => 'B',
            };
            format!("%{area}{size}{}", variable.offset)
        }
    }
}
