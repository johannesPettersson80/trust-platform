//! What a replay of a cycle trace needs from the trace and from the build that replays it
//! (specification 48 RU-60): the recorded tasks' names, whether the build lays its memory out as
//! the recorded program did, and the located outputs whose bytes differ after a cycle.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{section, ImageTables, IoArea, Located, TraceError};
use crate::native::image::{Image, Record, SectionId, TaskRecord, TABLE_PREFIX_BYTES};

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
        .filter(|variable| output_bits(variable, trace) != output_bits(variable, build))
        .collect()
}

/// The bytes of `variable` in the output image `image`, its bit alone for a variable of one bit;
/// `None` where the image does not hold it.
fn output_bits(variable: &Located, image: &[u8]) -> Option<Vec<u8>> {
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
