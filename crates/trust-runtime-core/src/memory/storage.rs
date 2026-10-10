use super::*;

impl VariableStorage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.globals.insert(name.into(), value);
    }

    #[must_use]
    pub fn globals(&self) -> &IndexMap<SmolStr, Value> {
        &self.globals
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<&Value> {
        self.globals.get(name)
    }

    pub fn set_retain(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.retain.insert(name.into(), value);
    }

    #[must_use]
    pub fn retain(&self) -> &IndexMap<SmolStr, Value> {
        &self.retain
    }

    #[must_use]
    pub fn get_retain(&self, name: &str) -> Option<&Value> {
        self.retain.get(name)
    }

    pub fn reset_runtime_values(&mut self, reset_instance_sequence: bool) {
        self.globals.clear();
        self.frames.clear();
        self.execution_frames.clear();
        self.instances.clear();
        self.next_frame_id = 0;
        if reset_instance_sequence {
            self.next_instance_id = 0;
        }
        cache::exclusive(&mut self.instance_field_offsets).clear();
        cache::exclusive(&mut self.recursive_instance_field_resolutions).clear();
    }
}

impl VariableStorage {
    /// Conservative collection-container charge for a snapshot, excluding values.
    /// This is a profile accounting bound, not measured allocator or board usage.
    pub fn clone_container_charge(&self) -> Option<usize> {
        let named_slot = core::mem::size_of::<(SmolStr, Value)>();
        let mut bytes = core::mem::size_of::<Self>();
        for len in [self.globals.len(), self.retain.len()] {
            bytes = bytes.checked_add(len.checked_mul(named_slot)?.checked_mul(4)?)?;
        }
        for instance in self.instances.values() {
            bytes = bytes.checked_add(core::mem::size_of::<InstanceData>().checked_mul(4)?)?;
            bytes = bytes.checked_add(
                instance
                    .variables
                    .len()
                    .checked_mul(named_slot)?
                    .checked_mul(4)?,
            )?;
        }
        for frame in &self.frames {
            bytes = bytes.checked_add(core::mem::size_of::<LocalFrame>())?;
            bytes = bytes.checked_add(
                frame
                    .variables
                    .len()
                    .checked_mul(named_slot)?
                    .checked_mul(4)?,
            )?;
        }
        bytes = bytes.checked_add(
            self.execution_frames
                .len()
                .checked_mul(core::mem::size_of::<(FrameId, Option<Vec<Value>>)>() * 4)?,
        )?;
        for values in self.execution_frames.values().flatten() {
            bytes = bytes.checked_add(values.len().checked_mul(core::mem::size_of::<Value>())?)?;
        }
        let direct =
            recover_read_lock(self.instance_field_offsets.read()).map_or(0, |cache| cache.len());
        let recursive = recover_read_lock(self.recursive_instance_field_resolutions.read())
            .map_or(0, |cache| cache.len());
        let declared = recover_read_lock(self.declared_instance_field_offsets.read())
            .map_or(0, |cache| cache.len());
        bytes = bytes.checked_add(
            direct
                .checked_mul(core::mem::size_of::<((InstanceId, SmolStr), Option<usize>)>())?
                .checked_mul(4)?,
        )?;
        bytes = bytes.checked_add(
            recursive
                .checked_mul(core::mem::size_of::<(
                    (InstanceId, SmolStr),
                    RecursiveInstanceFieldResolution,
                )>())?
                .checked_mul(4)?,
        )?;
        bytes.checked_add(
            declared
                .checked_mul(core::mem::size_of::<((SmolStr, SmolStr), usize)>())?
                .checked_mul(4)?,
        )
    }
}
