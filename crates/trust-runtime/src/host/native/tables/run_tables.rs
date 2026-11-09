//! The image's run tables (specification 44 §5): the entries, the I/O bindings, the tasks, and the
//! statements with their files. They live beside the other tables of `tables.rs`, which writes the
//! image in order.

use super::super::front::{first_token_start, last_token_end};
use super::super::model::Model;
use super::super::NativeError;
use super::{Inputs, Sorted};
use trust_runtime_core::io_address::IoSize;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::image::{
    EntryKind, EntryRecord, FileRecord, ImageWriter, IoBindRecord, StmtRecord, StringPool,
    TaskRecord, NONE,
};

/// The entry table, in the order of the generated entries: the program bodies by instance first;
/// a tool entry's index is its tool symbol's in `sorted` (§5.4).
pub(super) fn entries(
    writer: &mut ImageWriter,
    inputs: &Inputs<'_>,
    sorted: &Sorted,
) -> Result<(), NativeError> {
    let mut entries = Vec::with_capacity(inputs.generated.entries.len());
    for entry in &inputs.generated.entries {
        let Some(offset) = inputs.module.entries.get(&entry.symbol) else {
            return Err(NativeError::Module(format!(
                "no entry point {}",
                entry.symbol
            )));
        };
        let index = if entry.kind == EntryKind::Method {
            inputs
                .generated
                .tools
                .iter()
                .find(|tool| {
                    tool.block
                        .as_ref()
                        .is_some_and(|block| block.entry == entry.symbol)
                })
                .and_then(|tool| sorted.index(&tool.canonical))
                .ok_or_else(|| {
                    NativeError::Image(format!(
                        "the tool entry {} has no tool symbol",
                        entry.symbol
                    ))
                })?
        } else if let Some((canonical, copied)) = &entry.variable {
            // A change entry names its top-level variable's symbol: a change-exit or change-init
            // entry its index times two, plus one for the copied variable (§5.4).
            let symbol = sorted.index(canonical).ok_or_else(|| {
                NativeError::Image(format!(
                    "the change entry {} names {canonical}, which has no symbol",
                    entry.symbol
                ))
            })?;
            if entry.kind == EntryKind::ChangeReinit {
                symbol
            } else {
                symbol
                    .checked_mul(2)
                    .map(|index| index + u32::from(*copied))
                    .ok_or_else(|| {
                        NativeError::Image("too many symbols for change entries".into())
                    })?
            }
        } else {
            entry.index
        };
        entries.push(EntryRecord {
            kind: entry.kind as u32,
            index,
            code_offset: *offset,
        });
    }
    writer.table(&entries, &[]);
    Ok(())
}

/// The I/O bindings, each to its variable's symbol.
pub(super) fn bindings(
    writer: &mut ImageWriter,
    pool: &mut StringPool,
    model: &Model<'_>,
    sorted: &Sorted,
) -> Result<(), NativeError> {
    let mut records = Vec::with_capacity(model.bindings.len());
    for binding in &model.bindings {
        let canonical = binding
            .path
            .iter()
            .map(|part| part.to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join(".");
        let Some(symbol) = sorted.index(&canonical) else {
            return Err(NativeError::Compile(format!(
                "the binding of {} names no variable",
                binding.path.join(".")
            )));
        };
        let address = &binding.address;
        let hierarchical = address.path.len() > 1;
        records.push(IoBindRecord {
            symbol,
            area: match address.area {
                IoArea::Input => 1,
                IoArea::Output => 2,
                IoArea::Memory => 3,
            },
            size: match address.size {
                IoSize::Bit => 0,
                IoSize::Byte => 1,
                IoSize::Word => 2,
                IoSize::DWord => 3,
                IoSize::LWord => 4,
                IoSize::Bytes(_) => {
                    return Err(NativeError::unsupported("a byte window address".into()));
                }
            },
            // A hierarchical address is its text, which the kernel parses (§5.6).
            byte: if hierarchical {
                pool.add(&address.to_string())
            } else {
                address.byte
            },
            bit: address.bit,
            kind: if hierarchical {
                trust_runtime_core::native::image::BIND_HIERARCHICAL
            } else {
                trust_runtime_core::native::image::BIND_FLAT
            },
            flags: binding.codec
                | (binding.leaf << trust_runtime_core::native::image::BIND_OFFSET_SHIFT),
        });
    }
    writer.table(&records, &[]);
    Ok(())
}

/// The tasks with their program lists of entry indexes.
pub(super) fn tasks(
    writer: &mut ImageWriter,
    pool: &mut StringPool,
    model: &Model<'_>,
    inputs: &Inputs<'_>,
    sorted: &Sorted,
) {
    let mut records = Vec::with_capacity(model.tasks.len());
    let mut list = Vec::new();
    for task in &model.tasks {
        let first = u32::try_from(list.len() / 4).unwrap_or(u32::MAX);
        for program in &task.programs {
            let entry = inputs
                .generated
                .entries
                .iter()
                .position(|entry| {
                    entry.kind == EntryKind::ProgramBody && entry.index as usize == *program
                })
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(NONE);
            list.extend_from_slice(&entry.to_le_bytes());
        }
        // The function block instances it runs on their own, after its programs (IEC §6.8.2).
        for block in &task.blocks {
            let symbol = format!("trust_entry_b{block}");
            let entry = inputs
                .generated
                .entries
                .iter()
                .position(|entry| entry.symbol == symbol)
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(NONE);
            list.extend_from_slice(&entry.to_le_bytes());
        }
        records.push(TaskRecord {
            name: pool.add(&task.name),
            interval_ns: task.interval_ns,
            priority: task.priority,
            single: task
                .single
                .as_deref()
                .and_then(|name| sorted.index(&name.to_ascii_uppercase()))
                .unwrap_or(NONE),
            program_count: u32::try_from(task.programs.len() + task.blocks.len())
                .unwrap_or(u32::MAX),
            program_first: first,
            flags: 0,
        });
    }
    writer.table(&records, &list);
}

/// The statements in id order, with their files and positions, and the files.
pub(super) fn statements(writer: &mut ImageWriter, pool: &mut StringPool, inputs: &Inputs<'_>) {
    let mut records = Vec::with_capacity(inputs.ids.order.len());
    for (id, (file, node)) in inputs.ids.order.iter().enumerate() {
        let source = &inputs.front.sources[*file];
        let (start_line, start_column) = source.position(first_token_start(node));
        let (end_line, end_column) = source.position(last_token_end(node));
        records.push(StmtRecord {
            file: u16::try_from(*file).unwrap_or(u16::MAX),
            pou: inputs
                .generated
                .statement_pous
                .get(id)
                .copied()
                .unwrap_or(u16::MAX),
            start_line,
            start_column: u16::try_from(start_column).unwrap_or(u16::MAX),
            end_line,
            end_column: u16::try_from(end_column).unwrap_or(u16::MAX),
        });
    }
    writer.table(&records, &[]);
    let files: Vec<FileRecord> = inputs
        .front
        .sources
        .iter()
        .map(|source| FileRecord {
            label: pool.add(&source.label),
            source_offset: NONE,
            source_length: 0,
        })
        .collect();
    writer.table(&files, &[]);
}
