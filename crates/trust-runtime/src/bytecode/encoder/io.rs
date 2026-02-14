use crate::bytecode::{
    IoBinding, IoMap, ResourceEntry, ResourceMeta, RetainInit, RetainInitEntry, TaskEntry, VarMeta,
    VarMetaEntry,
};
use crate::io::IoTarget;
use crate::memory::IoArea;

use super::util::{format_io_address, to_u32};
use super::{BytecodeEncoder, BytecodeError};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn build_resource_meta(&mut self) -> Result<ResourceMeta, BytecodeError> {
        let name_idx = self.strings.intern("RESOURCE");
        let (inputs, outputs, memory) = process_image_sizes(self.runtime.io());
        let inputs_size = to_u32(inputs, "inputs size")?;
        let outputs_size = to_u32(outputs, "outputs size")?;
        let memory_size = to_u32(memory, "memory size")?;

        let mut tasks = Vec::new();
        for task in self.runtime.tasks() {
            let task_name_idx = self.strings.intern(task.name.clone());
            let single_name_idx = task
                .single
                .as_ref()
                .map(|name| self.strings.intern(name.clone()));
            let mut program_name_idx = Vec::new();
            for program in &task.programs {
                program_name_idx.push(self.strings.intern(program.clone()));
            }
            let mut fb_ref_idx = Vec::new();
            for reference in &task.fb_instances {
                fb_ref_idx.push(self.ref_index_for(reference)?);
            }
            tasks.push(TaskEntry {
                name_idx: task_name_idx,
                priority: task.priority,
                interval_nanos: task.interval.as_nanos(),
                single_name_idx,
                program_name_idx,
                fb_ref_idx,
            });
        }

        Ok(ResourceMeta {
            resources: vec![ResourceEntry {
                name_idx,
                inputs_size,
                outputs_size,
                memory_size,
                tasks,
            }],
        })
    }

    pub(super) fn build_io_map(&mut self) -> Result<IoMap, BytecodeError> {
        let mut bindings = Vec::new();
        for binding in self.runtime.io().bindings() {
            let address = format_io_address(&binding.address);
            let address_str_idx = self.strings.intern(address);
            let reference = match &binding.target {
                IoTarget::Reference(reference) => reference.clone(),
                IoTarget::Name(name) => self
                    .runtime
                    .storage()
                    .ref_for_global(name.as_ref())
                    .ok_or_else(|| BytecodeError::InvalidSection("unresolved IO binding".into()))?,
            };
            let ref_idx = self.ref_index_for(&reference)?;
            let type_id = binding
                .value_type
                .map(|type_id| self.type_index(type_id))
                .transpose()?;
            bindings.push(IoBinding {
                address_str_idx,
                ref_idx,
                type_id,
            });
        }
        Ok(IoMap { bindings })
    }

    pub(super) fn build_var_meta(&mut self) -> Result<VarMeta, BytecodeError> {
        let mut entries = Vec::new();
        for (name, meta) in self.runtime.globals() {
            let name_idx = self.strings.intern(name.clone());
            let type_id = self.type_index(meta.type_id)?;
            let reference = self
                .runtime
                .storage()
                .ref_for_global(name.as_ref())
                .ok_or_else(|| BytecodeError::InvalidSection("global reference missing".into()))?;
            let ref_idx = self.ref_index_for(&reference)?;
            let retain = match meta.retain {
                crate::RetainPolicy::Unspecified => 0,
                crate::RetainPolicy::Retain => 1,
                crate::RetainPolicy::NonRetain => 2,
                crate::RetainPolicy::Persistent => 3,
            };
            let init_const_idx = match &meta.init {
                crate::GlobalInitValue::Value(value) => self.const_index_for(value).ok(),
                _ => None,
            };
            entries.push(VarMetaEntry {
                name_idx,
                type_id,
                ref_idx,
                retain,
                init_const_idx,
            });
        }
        Ok(VarMeta { entries })
    }

    pub(super) fn build_retain_init(&self, meta: &VarMeta) -> Result<RetainInit, BytecodeError> {
        let mut entries = Vec::new();
        for entry in &meta.entries {
            if matches!(entry.retain, 1 | 3) {
                if let Some(const_idx) = entry.init_const_idx {
                    entries.push(RetainInitEntry {
                        ref_idx: entry.ref_idx,
                        const_idx,
                    });
                }
            }
        }
        Ok(RetainInit { entries })
    }
}

fn process_image_sizes(io: &crate::io::IoInterface) -> (usize, usize, usize) {
    let mut inputs = io.inputs().len();
    let mut outputs = io.outputs().len();
    let mut memory = io.memory().len();

    for binding in io.bindings() {
        let address = &binding.address;
        if address.wildcard || address.path.len() > 1 {
            continue;
        }
        let span = match address.size {
            crate::io::IoSize::Bit | crate::io::IoSize::Byte => 1usize,
            crate::io::IoSize::Word => 2usize,
            crate::io::IoSize::DWord => 4usize,
            crate::io::IoSize::LWord => 8usize,
        };
        let required = address.byte as usize + span;
        match address.area {
            IoArea::Input => inputs = inputs.max(required),
            IoArea::Output => outputs = outputs.max(required),
            IoArea::Memory => memory = memory.max(required),
        }
    }

    (inputs, outputs, memory)
}
