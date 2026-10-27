use super::*;

impl VariableStorage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The installation generation of this storage. Every reference it returns
    /// carries it, and its reference reads and writes resolve only references of
    /// this generation (specification 34 §11.3).
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.identities.generation
    }

    pub fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.values.globals.insert(name.into(), value);
    }

    #[must_use]
    pub fn globals(&self) -> &IndexMap<SmolStr, Value> {
        &self.values.globals
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<&Value> {
        self.values.globals.get(name)
    }

    pub fn set_retain(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.values.retain.insert(name.into(), value);
    }

    #[must_use]
    pub fn retain(&self) -> &IndexMap<SmolStr, Value> {
        &self.values.retain
    }

    #[must_use]
    pub fn get_retain(&self, name: &str) -> Option<&Value> {
        self.values.retain.get(name)
    }

    /// Clear values and identities in place. The generation is kept: this is the
    /// historical hosted fixture's restart, whose references stay valid across it.
    pub fn reset_runtime_values(&mut self, reset_instance_sequence: bool) {
        self.values.globals.clear();
        self.activations.frames.clear();
        self.activations.instances.clear();
        self.activations.next_frame_id = 0;
        if reset_instance_sequence {
            self.identities.next_instance_id = 0;
        }
        // No instance is live. A kept sequence is never reissued below its reset point.
        self.identities.start_entry_at_counter();
        #[cfg(feature = "std")]
        {
            cache::exclusive(&mut self.values.instance_field_offsets).clear();
            cache::exclusive(&mut self.values.recursive_instance_field_resolutions).clear();
        }
    }
}

impl VariableStorage {
    /// Conservative collection-container charge for a snapshot, excluding values.
    /// This is a profile accounting bound, not measured allocator or board usage.
    pub fn clone_container_charge(&self) -> Option<usize> {
        let named_slot = core::mem::size_of::<(SmolStr, Value)>();
        let mut bytes = core::mem::size_of::<Self>();
        for len in [self.values.globals.len(), self.values.retain.len()] {
            bytes = bytes.checked_add(len.checked_mul(named_slot)?.checked_mul(4)?)?;
        }
        for instance in self.activations.instances.values() {
            bytes = bytes.checked_add(core::mem::size_of::<InstanceData>().checked_mul(4)?)?;
            bytes = bytes.checked_add(
                instance
                    .variables
                    .len()
                    .checked_mul(named_slot)?
                    .checked_mul(4)?,
            )?;
        }
        for frame in &self.activations.frames {
            bytes = bytes.checked_add(core::mem::size_of::<LocalFrame>())?;
            bytes = bytes.checked_add(
                frame
                    .variables
                    .len()
                    .checked_mul(named_slot)?
                    .checked_mul(4)?,
            )?;
        }
        #[cfg(not(feature = "std"))]
        {
            // Clone preserves allocated vector capacity, including spare live slots.
            bytes = bytes.checked_add(
                self.activations
                    .instances
                    .capacity()
                    .checked_mul(core::mem::size_of::<(InstanceId, InstanceData)>())?,
            )?;
            Some(bytes)
        }
        #[cfg(feature = "std")]
        {
            let direct = recover_read_lock(self.values.instance_field_offsets.read())
                .map_or(0, |cache| cache.len());
            let recursive =
                recover_read_lock(self.values.recursive_instance_field_resolutions.read())
                    .map_or(0, |cache| cache.len());
            let declared = recover_read_lock(self.values.declared_instance_field_offsets.read())
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
}
