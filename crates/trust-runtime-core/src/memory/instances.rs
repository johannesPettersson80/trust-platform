use super::access::ref_for_map;
use super::*;

impl VariableStorage {
    /// Allocate an instance identity without wrapping the live reference namespace.
    /// An identity that is still live is refused, never replaced, in every composition.
    pub fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, crate::error::RuntimeError> {
        let (id, following) = self.identities.next_instance()?;
        self.insert_instance(id, type_name)?;
        self.identities.issue(id, following);
        Ok(id)
    }

    /// Record a new instance of `type_name` under the identity `id`, which the caller issued from the
    /// one counter of the store (specification 34 §6.2; R4). A live instance that holds `id` is refused,
    /// never replaced.
    pub(crate) fn insert_instance(
        &mut self,
        id: InstanceId,
        type_name: impl Into<SmolStr>,
    ) -> Result<(), crate::error::RuntimeError> {
        let value = InstanceData {
            type_name: type_name.into(),
            variables: IndexMap::default(),
            parent: None,
        };
        #[cfg(feature = "std")]
        {
            if self.activations.instances.contains_key(&id) {
                return Err(crate::error::RuntimeError::InvalidExecutionState);
            }
            self.activations.instances.insert(id, value);
        }
        #[cfg(not(feature = "std"))]
        self.activations.instances.try_append(id, value)?;
        Ok(())
    }

    /// Record `data` as the instance `id`, which the caller read from where another store holds it
    /// (C1-17). A live instance that holds `id` is refused, never replaced.
    pub(crate) fn put_instance(
        &mut self,
        id: InstanceId,
        data: InstanceData,
    ) -> Result<(), crate::error::RuntimeError> {
        self.insert_instance(id, data.type_name.clone())?;
        let held = self
            .get_instance_mut(id)
            .ok_or(crate::error::RuntimeError::InvalidExecutionState)?;
        *held = data;
        Ok(())
    }

    /// Array growth bytes and moved entries to charge before creating an instance.
    /// Hosted map nodes retain the existing per-instance construction accounting.
    pub fn instance_insertion_demand(&self) -> Option<(usize, usize)> {
        #[cfg(feature = "std")]
        {
            Some((0, 0))
        }
        #[cfg(not(feature = "std"))]
        {
            if self.activations.instances.len() < self.activations.instances.capacity() {
                return Some((0, 0));
            }
            let bytes = self
                .activations
                .instances
                .growth_capacity()?
                .checked_mul(core::mem::size_of::<(InstanceId, InstanceData)>())?;
            Some((bytes, self.activations.instances.len()))
        }
    }

    /// Release an invocation-owned instance and invalidate its lookup caches.
    pub fn remove_instance(&mut self, id: InstanceId) -> Option<InstanceData> {
        let removed = self.take_instance(id);
        if removed.is_some() {
            self.identities.release_instance(id);
        }
        removed
    }

    /// Take an instance record out and invalidate its lookup caches, leaving this storage's identities
    /// alone. The cell store takes its per-call instances out of this half and records the release in
    /// the one counter that issued them (specification 34 §6.2).
    pub(crate) fn take_instance(&mut self, id: InstanceId) -> Option<InstanceData> {
        self.invalidate_instance_field_caches(id);
        self.activations.instances.remove(&id)
    }

    /// Retire an owner batch in one pass. Callers charge the live entry count first.
    /// Cleanup itself cannot fail or consume additional execution fuel.
    pub fn retain_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        let floor = self.identities.entry_instance_floor;
        let (removed, released_since_floor) = self.retain_instances_above(floor, keep);
        self.identities.release(released_since_floor);
        removed
    }

    /// Retire an owner batch in one pass, answering how many instances were released and how many of
    /// them lay at or above `floor`, the entry floor of the counter that issued their identities. The
    /// cell store passes its own floor and records the release there (specification 34 §6.2).
    pub(crate) fn retain_instances_above(
        &mut self,
        floor: u32,
        mut keep: impl FnMut(InstanceId) -> bool,
    ) -> (usize, usize) {
        let before = self.activations.instances.len();
        let mut released_since_floor = 0usize;
        self.activations.instances.retain(|id, _| {
            let kept = keep(*id);
            if !kept && id.0 >= floor {
                released_since_floor += 1;
            }
            kept
        });
        let removed = before - self.activations.instances.len();
        #[cfg(feature = "std")]
        if removed != 0 {
            cache::exclusive(&mut self.values.instance_field_offsets).clear();
            cache::exclusive(&mut self.values.recursive_instance_field_resolutions).clear();
        }
        (removed, released_since_floor)
    }

    /// Test support only: panics when instance identities are exhausted. Every production
    /// construction path uses `try_create_instance`, which fails with `Overflow` instead
    /// (specification 34 HOST-02).
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_instance(&mut self, type_name: impl Into<SmolStr>) -> InstanceId {
        self.try_create_instance(type_name)
            .expect("instance identity or allocation exhausted")
    }

    /// Test support only: moves the next instance identity so the exhaustion path is reachable
    /// without creating 2^32 instances.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_next_instance_id_for_test(&mut self, next: u32) {
        self.identities.next_instance_id = next;
    }

    #[must_use]
    pub fn get_instance(&self, id: InstanceId) -> Option<&InstanceData> {
        self.activations.instances.get(&id)
    }

    #[must_use]
    pub fn instances(&self) -> &InstanceMap {
        &self.activations.instances
    }

    pub fn get_instance_mut(&mut self, id: InstanceId) -> Option<&mut InstanceData> {
        self.activations.instances.get_mut(&id)
    }

    pub fn set_instance_var(
        &mut self,
        id: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        let name = name.into();
        let is_new = if let Some(instance) = self.activations.instances.get_mut(&id) {
            let is_new = !instance.variables.contains_key(&name);
            instance.variables.insert(name, value);
            is_new
        } else {
            return false;
        };

        if is_new {
            self.invalidate_instance_field_caches(id);
        }
        true
    }

    #[must_use]
    pub fn get_instance_var(&self, id: InstanceId, name: &str) -> Option<&Value> {
        self.activations
            .instances
            .get(&id)
            .and_then(|instance| instance.variables.get(name))
    }

    #[must_use]
    pub fn get_instance_var_recursive(&self, id: InstanceId, name: &str) -> Option<&Value> {
        let mut current = Some(id);
        while let Some(instance_id) = current {
            if let Some(value) = self.get_instance_var(instance_id, name) {
                return Some(value);
            }
            current = self
                .activations
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
        }
        None
    }

    pub fn ref_for_global(&self, name: &str) -> Option<crate::value::ValueRef> {
        ref_for_map(
            &self.values.globals,
            MemoryLocation::Global,
            self.identities.generation,
            name,
        )
    }

    pub fn ref_for_local(&self, name: &str) -> Option<crate::value::ValueRef> {
        let frame = self.current_frame()?;
        ref_for_map(
            &frame.variables,
            MemoryLocation::Local(frame.id),
            self.identities.generation,
            name,
        )
    }

    pub fn ref_for_instance(&self, id: InstanceId, name: &str) -> Option<crate::value::ValueRef> {
        let offset = self.cached_instance_field_offset(id, name)?;
        self.instance_field_ref(id, offset)
    }

    /// A reference of this generation to one instance slot.
    fn instance_field_ref(&self, id: InstanceId, offset: usize) -> Option<crate::value::ValueRef> {
        Some(crate::value::ValueRef {
            location: MemoryLocation::Instance(id),
            offset: u32::try_from(offset).ok()?,
            generation: self.identities.generation,
            path: RefPath::new(),
        })
    }

    pub fn ref_for_instance_recursive(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let field_name = name;
        if let Some(resolution) = self.cached_recursive_instance_field_resolution(id, field_name) {
            let owner = self.resolve_ancestor_instance(id, resolution.owner_depth)?;
            return self.instance_field_ref(owner, resolution.offset);
        }

        let mut current = Some(id);
        let mut owner_depth = 0usize;
        while let Some(instance_id) = current {
            if let Some(offset) = self.cached_instance_field_offset(instance_id, field_name) {
                let resolution = RecursiveInstanceFieldResolution {
                    owner_depth,
                    offset,
                };
                self.cache_recursive_instance_field_resolution(id, field_name, resolution);
                return self.instance_field_ref(instance_id, offset);
            }
            current = self
                .activations
                .instances
                .get(&instance_id)
                .and_then(|instance| instance.parent);
            owner_depth += 1;
        }
        None
    }

    #[cfg(not(feature = "std"))]
    fn invalidate_instance_field_caches(&mut self, _: InstanceId) {}

    #[cfg(feature = "std")]
    fn invalidate_instance_field_caches(&mut self, id: InstanceId) {
        cache::exclusive(&mut self.values.instance_field_offsets)
            .retain(|(owner, _), _| *owner != id);
        cache::exclusive(&mut self.values.recursive_instance_field_resolutions)
            .retain(|(owner, _), _| *owner != id);
    }

    #[cfg(not(feature = "std"))]
    fn cached_instance_field_offset(&self, id: InstanceId, field_name: &str) -> Option<usize> {
        self.activations
            .instances
            .get(&id)?
            .variables
            .get_index_of(field_name)
    }

    #[cfg(feature = "std")]
    fn cached_instance_field_offset(&self, id: InstanceId, field_name: &str) -> Option<usize> {
        let key = (id, SmolStr::new(field_name));
        if let Some(cached) = recover_read_lock(self.values.instance_field_offsets.read())
            .and_then(|cache| cache.get(&key).copied())
        {
            return cached;
        }

        let offset = self
            .activations
            .instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index_of(field_name));
        if let Ok(mut cache) = self.values.instance_field_offsets.write() {
            cache.insert(key, offset);
        }
        offset
    }

    #[cfg(feature = "std")]
    fn cached_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &str,
    ) -> Option<RecursiveInstanceFieldResolution> {
        recover_read_lock(self.values.recursive_instance_field_resolutions.read())
            .and_then(|cache| cache.get(&(id, SmolStr::new(field_name))).copied())
    }

    #[cfg(feature = "std")]
    fn cache_recursive_instance_field_resolution(
        &self,
        id: InstanceId,
        field_name: &str,
        resolution: RecursiveInstanceFieldResolution,
    ) {
        if let Ok(mut cache) = self.values.recursive_instance_field_resolutions.write() {
            cache.insert((id, SmolStr::new(field_name)), resolution);
        }
    }

    #[cfg(not(feature = "std"))]
    fn cached_recursive_instance_field_resolution(
        &self,
        _: InstanceId,
        _: &str,
    ) -> Option<RecursiveInstanceFieldResolution> {
        None
    }

    #[cfg(not(feature = "std"))]
    fn cache_recursive_instance_field_resolution(
        &self,
        _: InstanceId,
        _: &str,
        _: RecursiveInstanceFieldResolution,
    ) {
    }

    fn resolve_ancestor_instance(&self, id: InstanceId, depth: usize) -> Option<InstanceId> {
        let mut current = id;
        for _ in 0..depth {
            current = self.activations.instances.get(&current)?.parent?;
        }
        Some(current)
    }

    #[cfg(not(feature = "std"))]
    pub fn declared_instance_field_offset(&self, id: InstanceId, name: &str) -> Option<usize> {
        self.activations
            .instances
            .get(&id)?
            .variables
            .get_index_of(name)
    }

    #[cfg(feature = "std")]
    pub fn declared_instance_field_offset(&self, id: InstanceId, name: &str) -> Option<usize> {
        let instance = self.activations.instances.get(&id)?;
        let field_name = SmolStr::new(name);
        let key = (instance.type_name.clone(), field_name.clone());
        if let Some(offset) = recover_read_lock(self.values.declared_instance_field_offsets.read())
            .and_then(|cache| cache.get(&key).copied())
        {
            return Some(offset);
        }

        let offset = instance.variables.get_index_of(field_name.as_str())?;
        if let Ok(mut cache) = self.values.declared_instance_field_offsets.write() {
            cache.insert(key, offset);
        }
        Some(offset)
    }

    pub fn declared_instance_field_ref(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        let offset = self.declared_instance_field_offset(id, name)?;
        self.instance_field_ref(id, offset)
    }

    pub fn resolved_instance_field_ref(
        &self,
        id: InstanceId,
        name: &str,
    ) -> Option<crate::value::ValueRef> {
        self.declared_instance_field_ref(id, name)
            .or_else(|| self.ref_for_instance_recursive(id, name))
    }

    pub fn read_instance_field_by_offset(&self, id: InstanceId, offset: usize) -> Option<&Value> {
        self.activations
            .instances
            .get(&id)
            .and_then(|instance| instance.variables.get_index(offset).map(|(_, value)| value))
    }

    pub fn write_instance_field_by_offset(
        &mut self,
        id: InstanceId,
        offset: usize,
        value: Value,
    ) -> bool {
        self.activations
            .instances
            .get_mut(&id)
            .and_then(|instance| {
                instance.variables.get_index_mut(offset).map(|(_, slot)| {
                    *slot = value;
                })
            })
            .is_some()
    }
}
