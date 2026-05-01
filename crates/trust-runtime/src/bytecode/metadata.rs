//! Bytecode metadata extraction.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::memory::{FrameId, InstanceId, IoArea, MemoryLocation};
use crate::task::TaskConfig;
use crate::value::{
    ref_indices_from_iter, Duration, RefPath, RefSegment as ValueRefSegment, ValueRef,
};

use super::{
    BytecodeError, BytecodeMetadata, BytecodeModule, ProcessImageConfig, RefEntry, RefLocation,
    RefSegment, RefTable, ResourceEntry, ResourceMetadata, SectionData, SectionId, StringTable,
};

impl BytecodeModule {
    pub fn metadata(&self) -> Result<BytecodeMetadata, BytecodeError> {
        let strings = match self.section(SectionId::StringTable) {
            Some(SectionData::StringTable(table)) => table,
            _ => return Err(BytecodeError::MissingSection("STRING_TABLE".into())),
        };
        let resource_meta = match self.section(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(meta)) => meta,
            _ => return Err(BytecodeError::MissingSection("RESOURCE_META".into())),
        };
        let ref_table = match self.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table,
            _ => return Err(BytecodeError::MissingSection("REF_TABLE".into())),
        };

        let resources = resource_meta
            .resources
            .iter()
            .map(|resource| resource_to_metadata(resource, strings, ref_table))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BytecodeMetadata {
            version: self.version,
            resources,
        })
    }
}

fn resource_to_metadata(
    resource: &ResourceEntry,
    strings: &StringTable,
    ref_table: &RefTable,
) -> Result<ResourceMetadata, BytecodeError> {
    let name = lookup_string(strings, resource.name_idx)?;
    let mut tasks = Vec::new();
    for task in &resource.tasks {
        let task_name = lookup_string(strings, task.name_idx)?;
        let programs = task
            .program_name_idx
            .iter()
            .map(|idx| lookup_string(strings, *idx))
            .collect::<Result<Vec<_>, _>>()?;
        let single = match task.single_name_idx {
            Some(idx) => Some(lookup_string(strings, idx)?),
            None => None,
        };
        let mut fb_instances = Vec::new();
        for idx in &task.fb_ref_idx {
            let entry = ref_table.entries.get(*idx as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "ref".into(),
                    index: *idx,
                }
            })?;
            fb_instances.push(ref_entry_to_value_ref(entry, strings)?);
        }
        tasks.push(TaskConfig {
            name: task_name,
            interval: Duration::from_nanos(task.interval_nanos),
            single,
            priority: task.priority,
            programs,
            fb_instances,
        });
    }

    Ok(ResourceMetadata {
        name,
        process_image: ProcessImageConfig {
            inputs: resource.inputs_size as usize,
            outputs: resource.outputs_size as usize,
            memory: resource.memory_size as usize,
        },
        tasks,
    })
}

fn ref_entry_to_value_ref(
    entry: &RefEntry,
    strings: &StringTable,
) -> Result<ValueRef, BytecodeError> {
    let location = match entry.location {
        RefLocation::Global => MemoryLocation::Global,
        RefLocation::Local => MemoryLocation::Local(FrameId(entry.owner_id)),
        RefLocation::Instance => MemoryLocation::Instance(InstanceId(entry.owner_id)),
        RefLocation::Retain => MemoryLocation::Retain,
        RefLocation::Io => {
            let area = match entry.owner_id {
                0 => IoArea::Input,
                1 => IoArea::Output,
                2 => IoArea::Memory,
                _ => return Err(BytecodeError::InvalidSection("invalid IO area".into())),
            };
            MemoryLocation::Io(area)
        }
    };
    let mut path = RefPath::new();
    for segment in &entry.segments {
        match segment {
            RefSegment::Index(indices) => {
                path.push(ValueRefSegment::Index(ref_indices_from_iter(
                    indices.iter().copied(),
                )));
            }
            RefSegment::Field { name_idx } => {
                let name = lookup_string(strings, *name_idx)?;
                path.push(ValueRefSegment::Field(name));
            }
        }
    }
    Ok(ValueRef {
        location,
        offset: entry.offset as usize,
        path,
    })
}

fn lookup_string(strings: &StringTable, idx: u32) -> Result<SmolStr, BytecodeError> {
    strings
        .entries
        .get(idx as usize)
        .cloned()
        .ok_or_else(|| BytecodeError::InvalidIndex {
            kind: "string".into(),
            index: idx,
        })
}
